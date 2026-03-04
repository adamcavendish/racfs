//! Cache wrapper for any FileSystem.
//!
//! Wraps an inner [`FileSystem`] and caches read, stat, and read_dir results
//! using the [`Cache`] trait. Mutations (create, write, remove, etc.) invalidate
//! the affected cache entries. Use with [`HashMapCache`] for tests or
//! [`FoyerCache`] for bounded production use.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use racfs_core::{
    Cache,
    error::FSError,
    filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS},
    flags::WriteFlags,
    metadata::FileMetadata,
};

/// A FileSystem that caches read, stat, and read_dir results behind a [`Cache`].
#[derive(Clone)]
pub struct CachedFs<C> {
    inner: Arc<dyn FileSystem>,
    cache: Arc<C>,
    /// Optional key prefix to avoid collisions when wrapping multiple mounts.
    key_prefix: String,
}

impl<C: Cache> CachedFs<C> {
    /// Create a new CachedFs wrapping `inner` and using `cache` for storage.
    pub fn new(inner: Arc<dyn FileSystem>, cache: Arc<C>) -> Self {
        Self {
            inner,
            cache,
            key_prefix: String::new(),
        }
    }

    /// Set a key prefix for cache keys (e.g. mount path). Use when multiple
    /// CachedFs share the same cache to avoid collisions.
    pub fn with_key_prefix(mut self, prefix: &str) -> Self {
        self.key_prefix = prefix.to_string();
        self
    }

    fn key(&self, kind: &str, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        format!("{}{}:{}", self.key_prefix, kind, path_str)
    }

    fn invalidate_path(&self, path: &Path) {
        let k_read = self.key("read", path);
        let k_stat = self.key("stat", path);
        let k_readdir = self.key("readdir", path);
        self.cache.remove(&k_read);
        self.cache.remove(&k_stat);
        self.cache.remove(&k_readdir);
        if let Some(parent) = path.parent()
            && parent != Path::new("")
        {
            let k_parent_dir = self.key("readdir", parent);
            self.cache.remove(&k_parent_dir);
        }
    }

    fn invalidate_prefix(&self, path: &Path) {
        let prefix = path.to_string_lossy();
        self.cache
            .invalidate_prefix(&format!("{}read:{}", self.key_prefix, prefix));
        self.cache
            .invalidate_prefix(&format!("{}stat:{}", self.key_prefix, prefix));
        self.cache
            .invalidate_prefix(&format!("{}readdir:{}", self.key_prefix, prefix));
        if let Some(parent) = path.parent()
            && parent != Path::new("")
        {
            let k_parent_dir = self.key("readdir", parent);
            self.cache.remove(&k_parent_dir);
        }
    }
}

#[async_trait]
impl<C: Cache + Send + Sync> ReadFS for CachedFs<C> {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let key = self.key("read", path);
        if let Some(data) = self.cache.get(&key) {
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset + size).min(data.len() as i64).max(0) as usize
            };
            if start >= data.len() {
                return Ok(Vec::new());
            }
            return Ok(data[start..end.min(data.len())].to_vec());
        }
        let data = self.inner.read(path, 0, -1).await?;
        self.cache.put(&key, &data);
        let start = offset.max(0) as usize;
        let end = if size < 0 {
            data.len()
        } else {
            (offset + size).min(data.len() as i64).max(0) as usize
        };
        if start >= data.len() {
            return Ok(Vec::new());
        }
        Ok(data[start..end.min(data.len())].to_vec())
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let key = self.key("stat", path);
        if let Some(bytes) = self.cache.get(&key)
            && let Ok(meta) = serde_json::from_slice(&bytes)
        {
            return Ok(meta);
        }
        let meta = self.inner.stat(path).await?;
        if let Ok(bytes) = serde_json::to_vec(&meta) {
            self.cache.put(&key, &bytes);
        }
        Ok(meta)
    }
}

#[async_trait]
impl<C: Cache + Send + Sync> WriteFS for CachedFs<C> {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        self.inner.create(path).await?;
        self.invalidate_path(path);
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let n = self.inner.write(path, data, offset, flags).await?;
        self.invalidate_path(path);
        Ok(n)
    }
}

#[async_trait]
impl<C: Cache + Send + Sync> DirFS for CachedFs<C> {
    async fn mkdir(&self, path: &Path, perm: u32) -> Result<(), FSError> {
        self.inner.mkdir(path, perm).await?;
        self.invalidate_path(path);
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let key = self.key("readdir", path);
        if let Some(bytes) = self.cache.get(&key)
            && let Ok(entries) = serde_json::from_slice(&bytes)
        {
            return Ok(entries);
        }
        let entries = self.inner.read_dir(path).await?;
        if let Ok(bytes) = serde_json::to_vec(&entries) {
            self.cache.put(&key, &bytes);
        }
        Ok(entries)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        self.inner.remove(path).await?;
        self.invalidate_path(path);
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        self.inner.remove_all(path).await?;
        self.invalidate_prefix(path);
        Ok(())
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        self.inner.rename(old_path, new_path).await?;
        self.invalidate_path(old_path);
        self.invalidate_path(new_path);
        Ok(())
    }
}

#[async_trait]
impl<C: Cache + Send + Sync> ChmodFS for CachedFs<C> {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        self.inner.chmod(path, mode).await?;
        self.invalidate_path(path);
        Ok(())
    }
}

#[async_trait]
impl<C: Cache + Send + Sync> FileSystem for CachedFs<C> {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let out = self.inner.truncate(path, size).await;
        if out.is_ok() {
            self.invalidate_path(path);
        }
        out
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let out = self.inner.touch(path).await;
        if out.is_ok() {
            self.invalidate_path(path);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CachedFs;
    use racfs_core::HashMapCache;
    use racfs_plugin_memfs::MemFS;
    use std::path::PathBuf;

    #[tokio::test]
    async fn cached_fs_delegates_and_caches_read() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        mem.write(&PathBuf::from("/f"), b"hello", 0, WriteFlags::none())
            .await
            .unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let data = cached.read(&PathBuf::from("/f"), 0, -1).await.unwrap();
        assert_eq!(data, b"hello");
        assert!(cache.get("read:/f").is_some());
        let data2 = cached.read(&PathBuf::from("/f"), 1, 3).await.unwrap();
        assert_eq!(data2, b"ell");
    }

    #[tokio::test]
    async fn cached_fs_invalidates_on_write() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        mem.write(&PathBuf::from("/f"), b"old", 0, WriteFlags::none())
            .await
            .unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let _ = cached.read(&PathBuf::from("/f"), 0, -1).await.unwrap();
        assert!(cache.get("read:/f").is_some());
        cached
            .write(&PathBuf::from("/f"), b"new", 0, WriteFlags::none())
            .await
            .unwrap();
        assert!(cache.get("read:/f").is_none());
        let data = cached.read(&PathBuf::from("/f"), 0, -1).await.unwrap();
        assert_eq!(data, b"new");
    }

    #[tokio::test]
    async fn cached_fs_caches_stat() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let _ = cached.stat(&PathBuf::from("/f")).await.unwrap();
        assert!(cache.get("stat:/f").is_some());
    }

    #[tokio::test]
    async fn cached_fs_with_key_prefix() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        mem.write(&PathBuf::from("/f"), b"x", 0, WriteFlags::none())
            .await
            .unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone()).with_key_prefix("m1:");
        let _ = cached.read(&PathBuf::from("/f"), 0, -1).await.unwrap();
        assert!(cache.get("m1:read:/f").is_some());
    }

    #[tokio::test]
    async fn cached_fs_remove_all_invalidates_prefix() {
        let mem = Arc::new(MemFS::new());
        mem.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
        mem.create(&PathBuf::from("/dir/a")).await.unwrap();
        mem.create(&PathBuf::from("/dir/b")).await.unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let _ = cached.read_dir(&PathBuf::from("/dir")).await.unwrap();
        assert!(cache.get("readdir:/dir").is_some());
        cached.remove_all(&PathBuf::from("/dir")).await.unwrap();
        assert!(cache.get("readdir:/dir").is_none());
    }

    #[tokio::test]
    async fn cached_fs_invalidates_on_mkdir_and_remove() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let _ = cached.stat(&PathBuf::from("/f")).await.unwrap();
        assert!(cache.get("stat:/f").is_some());
        cached.remove(&PathBuf::from("/f")).await.unwrap();
        assert!(cache.get("stat:/f").is_none());
    }

    #[tokio::test]
    async fn cached_fs_read_offset_beyond_len_returns_empty() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/short")).await.unwrap();
        mem.write(&PathBuf::from("/short"), b"hi", 0, WriteFlags::none())
            .await
            .unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let empty = cached.read(&PathBuf::from("/short"), 10, 5).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn cached_fs_truncate_invalidates_read_and_stat() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        mem.write(&PathBuf::from("/f"), b"hello", 0, WriteFlags::none())
            .await
            .unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let _ = cached.read(&PathBuf::from("/f"), 0, -1).await.unwrap();
        assert!(cache.get("read:/f").is_some());
        let _ = cached.stat(&PathBuf::from("/f")).await.unwrap();
        assert!(cache.get("stat:/f").is_some());
        cached.truncate(&PathBuf::from("/f"), 0).await.unwrap();
        assert!(cache.get("read:/f").is_none());
        assert!(cache.get("stat:/f").is_none());
        let data = cached.read(&PathBuf::from("/f"), 0, -1).await.unwrap();
        assert!(data.is_empty());
    }

    #[tokio::test]
    async fn cached_fs_touch_invalidates_stat() {
        let mem = Arc::new(MemFS::new());
        mem.create(&PathBuf::from("/f")).await.unwrap();
        let cache = Arc::new(HashMapCache::new());
        let cached = CachedFs::new(mem.clone(), cache.clone());
        let _ = cached.stat(&PathBuf::from("/f")).await.unwrap();
        assert!(cache.get("stat:/f").is_some());
        cached.touch(&PathBuf::from("/f")).await.unwrap();
        assert!(cache.get("stat:/f").is_none());
        let meta = cached.stat(&PathBuf::from("/f")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/f"));
    }
}
