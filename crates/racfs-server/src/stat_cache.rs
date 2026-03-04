//! Server-side stat cache with hit/miss metrics.
//!
//! Used to reduce VFS stat calls and expose cache hit/miss in Prometheus.
//! TTL is reloadable via SIGHUP (see config hot reload).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use prometheus::IntCounter;
use racfs_core::metadata::FileMetadata;

#[allow(dead_code)]
const DEFAULT_TTL_SECS: u64 = 2;

/// In-memory stat cache with TTL and hit/miss counters.
pub struct StatCache {
    entries: RwLock<std::collections::HashMap<PathBuf, (FileMetadata, Instant)>>,
    ttl: Arc<RwLock<Duration>>,
    hits: IntCounter,
    misses: IntCounter,
    evictions: IntCounter,
}

impl StatCache {
    /// Create with default TTL (2 seconds). Prefer `new_with_ttl` when loading from config.
    #[deprecated(
        since = "0.1.0",
        note = "Use StatCache::new_with_ttl for explicit TTL and eviction counter; required when using server config."
    )]
    #[allow(dead_code)]
    pub fn new(hits: IntCounter, misses: IntCounter, evictions: IntCounter) -> Self {
        Self::new_with_ttl(
            hits,
            misses,
            evictions,
            Duration::from_secs(DEFAULT_TTL_SECS),
        )
    }

    /// Create with a specific TTL (e.g. from config). TTL is stored in Arc<RwLock> for hot reload.
    pub fn new_with_ttl(
        hits: IntCounter,
        misses: IntCounter,
        evictions: IntCounter,
        ttl: Duration,
    ) -> Self {
        Self {
            entries: RwLock::new(std::collections::HashMap::new()),
            ttl: Arc::new(RwLock::new(ttl)),
            hits,
            misses,
            evictions,
        }
    }

    /// Update TTL (e.g. on SIGHUP reload). New TTL applies to newly cached entries.
    pub fn set_ttl(&self, ttl: Duration) {
        *self.ttl.write() = ttl;
    }

    /// Get cached stat or fetch and cache. On fetch error, the error is returned and miss is still incremented.
    pub async fn get_or_fetch<E, F, Fut>(&self, path: &Path, fetch: F) -> Result<FileMetadata, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<FileMetadata, E>>,
    {
        let key = path.to_path_buf();
        let ttl = *self.ttl.read();
        {
            let mut guard = self.entries.write();
            if let Some((meta, expires)) = guard.get(&key) {
                if Instant::now() < *expires {
                    self.hits.inc();
                    return Ok(meta.clone());
                }
                // Expired: remove and count as eviction
                guard.remove(&key);
                self.evictions.inc();
            }
        }
        self.misses.inc();
        let meta = fetch().await?;
        let expires = Instant::now() + ttl;
        self.entries.write().insert(key, (meta.clone(), expires));
        Ok(meta)
    }

    /// Invalidate cache for a path and its parent directory.
    pub fn invalidate(&self, path: &Path) {
        let mut guard = self.entries.write();
        guard.remove(&path.to_path_buf());
        if let Some(parent) = path.parent() {
            guard.remove(&parent.to_path_buf());
        }
    }

    /// Invalidate for rename: both paths and their parents.
    pub fn invalidate_rename(&self, old_path: &Path, new_path: &Path) {
        self.invalidate(old_path);
        self.invalidate(new_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn make_counters() -> (IntCounter, IntCounter, IntCounter) {
        (
            IntCounter::new("test_hits", "hits").unwrap(),
            IntCounter::new("test_misses", "misses").unwrap(),
            IntCounter::new("test_evictions", "evictions").unwrap(),
        )
    }

    #[tokio::test]
    async fn get_or_fetch_miss_then_hit() {
        let (hits, misses, evictions) = make_counters();
        let cache = StatCache::new_with_ttl(
            hits.clone(),
            misses.clone(),
            evictions.clone(),
            Duration::from_secs(10),
        );
        let path = PathBuf::from("/memfs/foo");
        let meta = FileMetadata::file(path.clone(), 42);

        let result = cache
            .get_or_fetch(path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(meta.clone())
            })
            .await
            .unwrap();
        assert_eq!(result.size, 42);
        assert_eq!(misses.get(), 1);
        assert_eq!(hits.get(), 0);

        let result2 = cache
            .get_or_fetch(path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(meta.clone())
            })
            .await
            .unwrap();
        assert_eq!(result2.size, 42);
        assert_eq!(misses.get(), 1);
        assert_eq!(hits.get(), 1);
    }

    #[tokio::test]
    async fn get_or_fetch_expired_evicts_and_refetches() {
        let (hits, misses, evictions) = make_counters();
        let cache = StatCache::new_with_ttl(
            hits.clone(),
            misses.clone(),
            evictions.clone(),
            Duration::from_millis(10),
        );
        let path = PathBuf::from("/memfs/expire_me");
        let meta0 = FileMetadata::file(path.clone(), 0);

        cache
            .get_or_fetch(path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(meta0)
            })
            .await
            .unwrap();
        assert_eq!(misses.get(), 1);
        assert_eq!(evictions.get(), 0);

        tokio::time::sleep(Duration::from_millis(25)).await;
        let meta1 = FileMetadata::file(path.clone(), 1);
        let result = cache
            .get_or_fetch(path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(meta1)
            })
            .await
            .unwrap();
        assert_eq!(result.size, 1);
        assert_eq!(misses.get(), 2);
        assert_eq!(evictions.get(), 1);
    }

    #[tokio::test]
    async fn get_or_fetch_propagates_fetch_error() {
        let (hits, misses, evictions) = make_counters();
        let cache = StatCache::new_with_ttl(
            hits.clone(),
            misses.clone(),
            evictions.clone(),
            Duration::from_secs(10),
        );
        let path = PathBuf::from("/memfs/fail");

        let err = cache
            .get_or_fetch(path.as_path(), || async {
                Err::<FileMetadata, io::Error>(io::Error::new(
                    io::ErrorKind::NotFound,
                    "file not found",
                ))
            })
            .await
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(misses.get(), 1);
        assert_eq!(hits.get(), 0);
    }

    #[tokio::test]
    async fn invalidate_clears_cache() {
        let (hits, misses, evictions) = make_counters();
        let cache = StatCache::new_with_ttl(
            hits.clone(),
            misses.clone(),
            evictions.clone(),
            Duration::from_secs(10),
        );
        let path = PathBuf::from("/memfs/bar");
        let meta = FileMetadata::file(path.clone(), 0);

        cache
            .get_or_fetch(path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(meta)
            })
            .await
            .unwrap();
        assert_eq!(misses.get(), 1);

        cache.invalidate(path.as_path());
        cache
            .get_or_fetch(path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(FileMetadata::file(path.clone(), 1))
            })
            .await
            .unwrap();
        assert_eq!(misses.get(), 2);
    }

    #[tokio::test]
    async fn invalidate_rename_clears_both_paths() {
        let (hits, misses, evictions) = make_counters();
        let cache = StatCache::new_with_ttl(
            hits.clone(),
            misses.clone(),
            evictions.clone(),
            Duration::from_secs(10),
        );
        let old_path = PathBuf::from("/memfs/old");
        let new_path = PathBuf::from("/memfs/new");
        cache
            .get_or_fetch(old_path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(FileMetadata::file(old_path.clone(), 0))
            })
            .await
            .unwrap();
        cache
            .get_or_fetch(new_path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(FileMetadata::file(new_path.clone(), 0))
            })
            .await
            .unwrap();
        assert_eq!(misses.get(), 2);

        cache.invalidate_rename(old_path.as_path(), new_path.as_path());
        cache
            .get_or_fetch(old_path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(FileMetadata::file(old_path.clone(), 1))
            })
            .await
            .unwrap();
        cache
            .get_or_fetch(new_path.as_path(), || async {
                Ok::<FileMetadata, io::Error>(FileMetadata::file(new_path.clone(), 1))
            })
            .await
            .unwrap();
        assert_eq!(misses.get(), 4);
    }

    #[test]
    fn set_ttl_does_not_panic() {
        let (hits, misses, evictions) = make_counters();
        let cache = StatCache::new_with_ttl(hits, misses, evictions, Duration::from_secs(2));
        cache.set_ttl(Duration::from_secs(5));
    }
}
