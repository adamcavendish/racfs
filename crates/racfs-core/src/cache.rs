//! Cache trait abstraction for RACFS.
//!
//! Provides a common interface for pluggable caches (in-memory, disk, or
//! hybrid e.g. Foyer). Used by plugins (e.g. S3FS) and the FUSE client for
//! metadata and file content caching.
//!
//! Implementors may optionally report statistics for hit rate and eviction
//! monitoring via [`Cache::stats`].

use std::collections::HashMap;
use std::sync::Mutex;

/// Statistics for cache monitoring (hit rate, eviction count).
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of successful lookups.
    pub hits: u64,
    /// Number of lookups that missed.
    pub misses: u64,
    /// Number of entries evicted (e.g. by capacity or TTL).
    pub evictions: u64,
}

impl CacheStats {
    /// Total number of lookups (hits + misses).
    pub fn total_requests(&self) -> u64 {
        self.hits + self.misses
    }

    /// Hit rate in [0.0, 1.0] if there were any requests; otherwise 0.
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Abstraction for key-value caches used by filesystem layers.
///
/// Keys are strings (e.g. path or path+offset); values are byte buffers.
/// Implementations may be in-memory (e.g. `HashMap`), disk-based, or hybrid
/// (e.g. Foyer). Optional statistics support monitoring and tuning.
pub trait Cache: Send + Sync {
    /// Returns the value for `key`, or `None` if missing or expired.
    fn get(&self, key: &str) -> Option<Vec<u8>>;

    /// Inserts or overwrites the value for `key`.
    fn put(&self, key: &str, value: &[u8]);

    /// Removes the entry for `key`. No-op if the key is not present.
    fn remove(&self, key: &str);

    /// Removes all entries whose key starts with `prefix` (e.g. path-based
    /// invalidation). Default implementation does nothing; override for
    /// prefix-aware caches.
    fn invalidate_prefix(&self, prefix: &str) {
        let _ = prefix;
    }

    /// Returns current cache statistics if the implementation tracks them.
    /// Default returns `None`.
    fn stats(&self) -> Option<CacheStats> {
        None
    }
}

/// Simple in-memory cache backed by a `HashMap`.
///
/// No capacity limit, TTL, or eviction policy. Useful for tests and
/// as a reference implementation. For production use, use [`FoyerCache`]
/// for bounded capacity and (optionally) hybrid memory+disk caching.
#[derive(Debug, Default)]
pub struct HashMapCache {
    inner: Mutex<HashMap<String, Vec<u8>>>,
}

impl HashMapCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Cache for HashMapCache {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.inner.lock().unwrap().get(key).cloned()
    }

    fn put(&self, key: &str, value: &[u8]) {
        self.inner
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_vec());
    }

    fn remove(&self, key: &str) {
        self.inner.lock().unwrap().remove(key);
    }

    fn invalidate_prefix(&self, prefix: &str) {
        let mut guard = self.inner.lock().unwrap();
        let keys: Vec<String> = guard
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        for k in keys {
            guard.remove(&k);
        }
    }
}

/// Bounded in-memory cache backed by [foyer](https://docs.rs/foyer) with configurable capacity and
/// eviction (e.g. LRU, LFU, S3Fifo). Use for production when you need bounded capacity; supports
/// hybrid (memory + disk) via foyer's `HybridCache` for cost reduction (e.g. S3 grepping).
///
/// Note: [`invalidate_prefix`](Cache::invalidate_prefix) is a no-op (foyer does not support
/// prefix iteration). For path-based invalidation use [`HashMapCache`] or track keys externally.
#[derive(Debug)]
pub struct FoyerCache {
    inner: std::sync::Arc<foyer::Cache<String, Vec<u8>>>,
}

impl FoyerCache {
    /// Create a new foyer in-memory cache with the given capacity (max number of entries).
    pub fn new(capacity: usize) -> Self {
        let inner = foyer::Cache::builder(capacity).build();
        Self {
            inner: std::sync::Arc::new(inner),
        }
    }
}

impl Cache for FoyerCache {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.inner.get(key).map(|e| e.value().to_vec())
    }

    fn put(&self, key: &str, value: &[u8]) {
        self.inner.insert(key.to_string(), value.to_vec());
    }

    fn remove(&self, key: &str) {
        self.inner.remove(key);
    }

    fn invalidate_prefix(&self, prefix: &str) {
        let _ = prefix;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashmap_cache_get_put_remove() {
        let c = HashMapCache::new();
        assert!(c.get("a").is_none());
        c.put("a", b"hello");
        assert_eq!(c.get("a").as_deref(), Some(b"hello" as &[u8]));
        c.remove("a");
        assert!(c.get("a").is_none());
    }

    #[test]
    fn test_hashmap_cache_invalidate_prefix() {
        let c = HashMapCache::new();
        c.put("/path/a", b"1");
        c.put("/path/b", b"2");
        c.put("/other", b"3");
        c.invalidate_prefix("/path");
        assert!(c.get("/path/a").is_none());
        assert!(c.get("/path/b").is_none());
        assert_eq!(c.get("/other").as_deref(), Some(b"3" as &[u8]));
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let s = CacheStats {
            hits: 80,
            misses: 20,
            evictions: 5,
        };
        assert_eq!(s.total_requests(), 100);
        assert!((s.hit_rate() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_cache_stats_empty() {
        let s = CacheStats::default();
        assert_eq!(s.total_requests(), 0);
        assert_eq!(s.hit_rate(), 0.0);
    }

    #[test]
    fn test_hashmap_cache_returns_no_stats() {
        let c = HashMapCache::new();
        assert!(c.stats().is_none());
    }

    #[test]
    fn test_cache_stats_with_evictions() {
        let s = CacheStats {
            hits: 10,
            misses: 5,
            evictions: 2,
        };
        assert_eq!(s.total_requests(), 15);
        assert!((s.hit_rate() - 10.0 / 15.0).abs() < 1e-9);
    }

    #[test]
    fn test_foyer_cache_get_put_remove() {
        let c = super::FoyerCache::new(16);
        assert!(c.get("a").is_none());
        c.put("a", b"hello");
        assert_eq!(c.get("a").as_deref(), Some(b"hello" as &[u8]));
        c.remove("a");
        assert!(c.get("a").is_none());
    }
}
