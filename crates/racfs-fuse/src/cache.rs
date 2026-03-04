//! TTL-based client cache for FUSE metadata, directory listings, and file content.
//!
//! Reduces server round-trips for repeated lookup/getattr/readdir and **sequential
//! reads**: file content is cached after the first read so subsequent reads (any
//! offset) are served from the buffer without a network call. Cache is invalidated
//! on write operations.
//!
//! **Write-back buffer:** Small writes (≤ `DEFAULT_WRITE_BACK_MAX_CHUNK`) are
//! accumulated per path and flushed on file release or when total buffered size
//! exceeds `DEFAULT_WRITE_BACK_FLUSH_SIZE`.

use bytes::Bytes;
use dashmap::DashMap;
use racfs_client::types::{DirectoryListResponse, FileMetadataResponse};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Default TTL for cached entries (seconds).
pub const DEFAULT_TTL_SECS: u64 = 1;

/// Max size of a single write to buffer (writes larger than this go straight to server).
pub const DEFAULT_WRITE_BACK_MAX_CHUNK: usize = 64 * 1024; // 64 KiB

/// Flush write-back buffer when total buffered bytes per file exceeds this.
pub const DEFAULT_WRITE_BACK_FLUSH_SIZE: usize = 256 * 1024; // 256 KiB

/// Entry with expiry for metadata cache.
#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

/// Pending write segments for write-back buffer (offset, data).
struct WriteBackEntry {
    segments: Vec<(u64, Vec<u8>)>,
    total_bytes: usize,
}

impl WriteBackEntry {
    fn push(&mut self, offset: u64, data: Vec<u8>) -> usize {
        let len = data.len();
        self.segments.push((offset, data));
        self.total_bytes += len;
        self.total_bytes
    }

    fn take_segments(&mut self) -> Vec<(u64, Vec<u8>)> {
        self.total_bytes = 0;
        std::mem::take(&mut self.segments)
    }
}

/// TTL cache for stat (metadata), readdir, and file content (read-ahead buffer).
pub struct FuseCache {
    ttl: Duration,
    stat_cache: DashMap<PathBuf, CacheEntry<FileMetadataResponse>>,
    readdir_cache: DashMap<PathBuf, CacheEntry<DirectoryListResponse>>,
    /// Cached file content for sequential/repeated reads. Invalidated on write.
    file_content_cache: DashMap<PathBuf, Bytes>,
    /// Write-back buffer: pending small writes per path, flushed on release or when over threshold.
    write_back: DashMap<PathBuf, Mutex<WriteBackEntry>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl FuseCache {
    /// Create a new cache with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            stat_cache: DashMap::new(),
            readdir_cache: DashMap::new(),
            file_content_cache: DashMap::new(),
            write_back: DashMap::new(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Create cache with default TTL (1 second).
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(DEFAULT_TTL_SECS))
    }

    /// Get cached stat if present and not expired.
    pub fn get_stat(&self, path: &Path) -> Option<FileMetadataResponse> {
        let key = path.to_path_buf();
        let entry = match self.stat_cache.get(&key) {
            Some(e) => e,
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        if entry.is_valid() {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(entry.value.clone())
        } else {
            drop(entry);
            self.stat_cache.remove(&key);
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Get cached stat if present, even if expired (for graceful degradation when backend is unreachable).
    pub fn get_stat_stale(&self, path: &Path) -> Option<FileMetadataResponse> {
        let key = path.to_path_buf();
        self.stat_cache.get(&key).map(|e| e.value.clone())
    }

    /// Store stat in cache.
    pub fn put_stat(&self, path: &Path, metadata: FileMetadataResponse) {
        let key = path.to_path_buf();
        self.stat_cache
            .insert(key, CacheEntry::new(metadata, self.ttl));
    }

    /// Get cached readdir if present and not expired.
    pub fn get_readdir(&self, path: &Path) -> Option<DirectoryListResponse> {
        let key = path.to_path_buf();
        let entry = match self.readdir_cache.get(&key) {
            Some(e) => e,
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        if entry.is_valid() {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(entry.value.clone())
        } else {
            drop(entry);
            self.readdir_cache.remove(&key);
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Get cached readdir if present, even if expired (for graceful degradation when backend is unreachable).
    pub fn get_readdir_stale(&self, path: &Path) -> Option<DirectoryListResponse> {
        let key = path.to_path_buf();
        self.readdir_cache.get(&key).map(|e| e.value.clone())
    }

    /// Store readdir result in cache.
    pub fn put_readdir(&self, path: &Path, list: DirectoryListResponse) {
        let key = path.to_path_buf();
        self.readdir_cache
            .insert(key, CacheEntry::new(list, self.ttl));
    }

    /// Invalidate cache for a path and its parent (so listing parent reflects changes).
    pub fn invalidate(&self, path: &Path) {
        let key = path.to_path_buf();
        self.stat_cache.remove(&key);
        self.readdir_cache.remove(&key);
        self.file_content_cache.remove(&key);
        self.write_back.remove(&key);
        if let Some(parent) = path.parent() {
            let parent_key = parent.to_path_buf();
            self.readdir_cache.remove(&parent_key);
        }
    }

    /// Invalidate for rename: old path, new path, and their parents.
    pub fn invalidate_rename(&self, old_path: &Path, new_path: &Path) {
        self.invalidate(old_path);
        self.invalidate(new_path);
        if let Some(p) = new_path.parent() {
            self.readdir_cache.remove(&p.to_path_buf());
        }
        self.file_content_cache.remove(&old_path.to_path_buf());
        self.file_content_cache.remove(&new_path.to_path_buf());
        self.write_back.remove(&old_path.to_path_buf());
        self.write_back.remove(&new_path.to_path_buf());
    }

    /// Get cached file content if present. Used for read-ahead/sequential reads.
    pub fn get_file_content(&self, path: &Path) -> Option<Bytes> {
        let key = path.to_path_buf();
        self.file_content_cache.get(&key).map(|v| v.clone())
    }

    /// Store file content in cache (e.g. after first read for sequential serving).
    pub fn put_file_content(&self, path: &Path, data: impl Into<Bytes>) {
        let key = path.to_path_buf();
        self.file_content_cache.insert(key, data.into());
    }

    /// Invalidate cached file content for a path (on write/truncate/unlink/rename).
    pub fn invalidate_file_content(&self, path: &Path) {
        self.file_content_cache.remove(&path.to_path_buf());
    }

    /// Push a small write into the write-back buffer for the given path.
    /// Returns total buffered bytes after this push (caller may flush when >= threshold).
    pub fn push_write_back(&self, path: &Path, offset: u64, data: Vec<u8>) -> usize {
        let key = path.to_path_buf();
        self.write_back
            .entry(key)
            .or_insert_with(|| {
                Mutex::new(WriteBackEntry {
                    segments: Vec::new(),
                    total_bytes: 0,
                })
            })
            .lock()
            .unwrap()
            .push(offset, data)
    }

    /// Remove and return pending write segments for the path (for flush). Returns empty vec if none.
    pub fn take_pending_writes(&self, path: &Path) -> Vec<(u64, Vec<u8>)> {
        let key = path.to_path_buf();
        if let Some((_, mutex_entry)) = self.write_back.remove(&key) {
            let mut entry = mutex_entry.lock().unwrap();
            return entry.take_segments();
        }
        Vec::new()
    }

    /// Cache hit count (for metrics).
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Cache miss count (for metrics).
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

impl Default for FuseCache {
    fn default() -> Self {
        Self::with_default_ttl()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_cache_miss_then_hit() {
        let cache = FuseCache::new(Duration::from_secs(10));
        let path = Path::new("/foo");
        assert!(cache.get_stat(path).is_none());
        assert_eq!(cache.misses(), 1);

        let meta = FileMetadataResponse {
            file_type: "file".to_string(),
            permissions: "rw-r--r--".to_string(),
            size: 0,
            modified: None,
            path: "/foo".to_string(),
            symlink_target: None,
        };
        cache.put_stat(path, meta);
        let cached = cache.get_stat(path);
        assert!(cached.is_some());
        assert_eq!(cached.as_ref().unwrap().path, "/foo");
        assert_eq!(cache.hits(), 1);
    }

    #[test]
    fn test_invalidate() {
        let cache = FuseCache::new(Duration::from_secs(10));
        let path = Path::new("/a/b");
        cache.put_stat(
            path,
            FileMetadataResponse {
                file_type: "file".to_string(),
                permissions: "rw-".to_string(),
                size: 0,
                modified: None,
                path: "/a/b".to_string(),
                symlink_target: None,
            },
        );
        cache.invalidate(path);
        assert!(cache.get_stat(path).is_none());
    }

    #[test]
    fn test_file_content_cache_read_ahead() {
        let cache = FuseCache::new(Duration::from_secs(10));
        let path = Path::new("/memfs/foo.txt");
        assert!(cache.get_file_content(path).is_none());

        cache.put_file_content(path, Bytes::from_static(b"hello world"));
        let data = cache.get_file_content(path).unwrap();
        assert_eq!(data.as_ref(), b"hello world");

        cache.invalidate_file_content(path);
        assert!(cache.get_file_content(path).is_none());
    }

    #[test]
    fn test_write_back_buffer() {
        let cache = FuseCache::new(Duration::from_secs(10));
        let path = Path::new("/memfs/writeback.txt");

        assert!(cache.take_pending_writes(path).is_empty());

        let n1 = cache.push_write_back(path, 0, b"hello".to_vec());
        assert_eq!(n1, 5);
        let n2 = cache.push_write_back(path, 5, b" world".to_vec());
        assert_eq!(n2, 11);

        let pending = cache.take_pending_writes(path);
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0], (0, b"hello".to_vec()));
        assert_eq!(pending[1], (5, b" world".to_vec()));

        assert!(cache.take_pending_writes(path).is_empty());
        let n3 = cache.push_write_back(path, 0, b"again".to_vec());
        assert_eq!(n3, 5);
    }

    #[test]
    fn test_with_default_ttl() {
        let cache = FuseCache::with_default_ttl();
        let path = Path::new("/p");
        assert!(cache.get_stat(path).is_none());
    }

    #[test]
    fn test_default_impl() {
        let cache = FuseCache::default();
        assert_eq!(cache.misses(), 0);
        assert_eq!(cache.hits(), 0);
    }

    #[test]
    fn test_get_stat_stale_returns_expired_entry() {
        let cache = FuseCache::new(Duration::from_nanos(1));
        let path = Path::new("/stale");
        let meta = FileMetadataResponse {
            file_type: "file".to_string(),
            permissions: "rw-".to_string(),
            size: 42,
            modified: None,
            path: "/stale".to_string(),
            symlink_target: None,
        };
        cache.put_stat(path, meta.clone());
        std::thread::sleep(Duration::from_millis(2));
        let stale = cache.get_stat_stale(path);
        assert!(
            stale.is_some(),
            "get_stat_stale should return expired entry"
        );
        assert_eq!(stale.unwrap().size, 42);
        assert!(cache.get_stat(path).is_none());
    }

    #[test]
    fn test_get_readdir_stale_returns_expired_entry() {
        let cache = FuseCache::new(Duration::from_nanos(1));
        let path = Path::new("/dir");
        let list = DirectoryListResponse { entries: vec![] };
        cache.put_readdir(path, list);
        std::thread::sleep(Duration::from_millis(2));
        let stale = cache.get_readdir_stale(path);
        assert!(
            stale.is_some(),
            "get_readdir_stale should return expired entry"
        );
        assert!(stale.unwrap().entries.is_empty());
        assert!(cache.get_readdir(path).is_none());
    }
}
