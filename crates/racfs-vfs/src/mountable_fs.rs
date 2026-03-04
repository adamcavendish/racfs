//! Mountable filesystem with radix tree routing.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use parking_lot::RwLock;
use racfs_core::{
    error::FSError,
    filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS},
    flags::WriteFlags,
    metadata::FileMetadata,
};
use radix_trie::Trie;

use crate::handle_manager::HandleManager;

/// A mounted filesystem with its mount point path.
#[allow(dead_code)]
struct MountedFS {
    fs: Arc<dyn FileSystem>,
    mount_point: PathBuf,
}

/// MountableFS provides radix tree-based routing for multiple filesystem backends.
pub struct MountableFS {
    mounts: RwLock<HashMap<PathBuf, Arc<dyn FileSystem>>>,
    trie: RwLock<Trie<String, Arc<dyn FileSystem>>>,
    handle_manager: HandleManager,
}

impl MountableFS {
    /// Create a new MountableFS.
    pub fn new() -> Self {
        Self {
            mounts: RwLock::new(HashMap::new()),
            trie: RwLock::new(Trie::new()),
            handle_manager: HandleManager::new(),
        }
    }

    /// Get the handle manager.
    pub fn handle_manager(&self) -> &HandleManager {
        &self.handle_manager
    }

    /// Mount a filesystem at a path.
    ///
    /// The mount point must not already exist and must be a valid path.
    pub fn mount(&self, mount_point: PathBuf, fs: Arc<dyn FileSystem>) -> Result<(), FSError> {
        // Validate mount point
        if mount_point.as_os_str() == "/" {
            return Err(FSError::InvalidInput {
                message: "cannot mount at root".to_string(),
            });
        }

        if mount_point.components().any(|c| c.as_os_str().is_empty()) {
            return Err(FSError::InvalidInput {
                message: "invalid mount point".to_string(),
            });
        }

        // Check if mount point already exists
        {
            let mounts = self.mounts.read();
            if mounts.contains_key(&mount_point) {
                return Err(FSError::AlreadyExists { path: mount_point });
            }
        }

        // Normalize mount point (ensure trailing slash for proper routing)
        let normalized = Self::normalize_mount_point(&mount_point);

        // Add to hash map
        {
            let mut mounts = self.mounts.write();
            mounts.insert(normalized.clone(), fs.clone());
        }

        // Add to radix trie
        {
            let mut trie = self.trie.write();
            let key = Self::path_to_key(&normalized);
            trie.insert(key, fs);
        }

        tracing::info!(mount_point = %normalized.display(), "filesystem mounted");
        Ok(())
    }

    /// Unmount a filesystem.
    pub fn unmount(&self, mount_point: &Path) -> Result<(), FSError> {
        let normalized = Self::normalize_mount_point(mount_point);

        // Remove from hash map
        {
            let mut mounts = self.mounts.write();
            if mounts.remove(&normalized).is_none() {
                return Err(FSError::NotFound {
                    path: normalized.clone(),
                });
            }
        }

        // Remove from radix trie
        {
            let mut trie = self.trie.write();
            let key = Self::path_to_key(&normalized);
            trie.remove(&key);
        }

        tracing::info!(mount_point = %normalized.display(), "filesystem unmounted");
        Ok(())
    }

    /// Resolve the filesystem for a given path using radix trie for O(log n) lookup.
    pub fn resolve_fs(&self, path: &Path) -> Result<(Arc<dyn FileSystem>, PathBuf), FSError> {
        let normalized = Self::normalize_path(path);
        let key = Self::path_to_key(&normalized);

        // Use radix trie for efficient longest-prefix match
        // We need to iterate through possible ancestors to find the longest match
        let fs = {
            let trie = self.trie.read();
            let mut best_match: Option<Arc<dyn FileSystem>> = None;
            let mut best_len = 0;

            // Try progressively shorter prefixes to find the longest match
            for i in (0..=key.len()).rev() {
                let prefix = &key[..i];
                if let Some(fs_ptr) = trie.get(prefix) {
                    let len = prefix.len();
                    if len >= best_len {
                        best_len = len;
                        best_match = Some(fs_ptr.clone());
                    }
                }
            }

            best_match.ok_or_else(|| FSError::NotFound {
                path: normalized.clone(),
            })?
        };

        // Find the mount point for calculating relative path
        // Search through mounts to find the best match (longest prefix)
        let mount_point = {
            let mounts = self.mounts.read();
            let mut best_match: Option<PathBuf> = None;
            let mut best_len = 0;

            for (mount, _) in mounts.iter() {
                let mount_key = Self::path_to_key(mount);
                if key.starts_with(&mount_key) || mount_key.is_empty() {
                    let len = mount_key.len();
                    if len > best_len {
                        best_len = len;
                        best_match = Some(mount.clone());
                    }
                }
            }
            best_match.unwrap_or_else(|| PathBuf::from("/"))
        };

        // Calculate the relative path within the mounted filesystem
        let relative_path = if mount_point.as_os_str() == "/" {
            normalized.clone()
        } else {
            normalized
                .strip_prefix(&mount_point)
                .unwrap_or(&normalized)
                .to_path_buf()
        };

        // Ensure relative path starts with /
        let relative_path = if relative_path.as_os_str().is_empty() {
            PathBuf::from("/")
        } else if !relative_path.starts_with("/") {
            PathBuf::from("/").join(&relative_path)
        } else {
            relative_path
        };

        Ok((fs, relative_path))
    }

    /// List all mount points.
    pub fn list_mounts(&self) -> Vec<PathBuf> {
        let mounts = self.mounts.read();
        mounts.keys().cloned().collect()
    }

    /// Normalize a mount point (ensure trailing slash).
    fn normalize_mount_point(path: &Path) -> PathBuf {
        let mut p = path.to_path_buf();
        let s = p.to_string_lossy();
        if !s.ends_with('/') && p.as_os_str() != "/" {
            p.push("");
        }
        p
    }

    /// Normalize a path.
    fn normalize_path(path: &Path) -> PathBuf {
        let mut result = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    result.pop();
                }
                std::path::Component::CurDir => {}
                std::path::Component::RootDir => result.push("/"),
                std::path::Component::Normal(name) => result.push(name),
                std::path::Component::Prefix(prefix) => {
                    result.push(prefix.as_os_str());
                }
            }
        }
        if result.as_os_str().is_empty() {
            result.push("/");
        }
        result
    }

    /// Convert path to trie key.
    fn path_to_key(path: &Path) -> String {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_suffix('/') {
            stripped.to_string()
        } else {
            s.to_string()
        }
    }
}

impl Default for MountableFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ReadFS for MountableFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.read(&relative, offset, size).await
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.stat(&relative).await
    }
}

#[async_trait]
impl WriteFS for MountableFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.create(&relative).await
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.write(&relative, data, offset, flags).await
    }
}

#[async_trait]
impl DirFS for MountableFS {
    async fn mkdir(&self, path: &Path, perm: u32) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.mkdir(&relative, perm).await
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.read_dir(&relative).await
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.remove(&relative).await
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.remove_all(&relative).await
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let (fs_old, relative_old) = self.resolve_fs(old_path)?;
        let (fs_new, relative_new) = self.resolve_fs(new_path)?;

        // Check if both paths are on the same filesystem
        if !Arc::ptr_eq(&fs_old, &fs_new) {
            return Err(FSError::CrossDeviceLink);
        }

        fs_old.rename(&relative_old, &relative_new).await
    }
}

#[async_trait]
impl ChmodFS for MountableFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.chmod(&relative, mode).await
    }
}

#[async_trait]
impl FileSystem for MountableFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.truncate(&relative, size).await
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.touch(&relative).await
    }

    async fn readlink(&self, path: &Path) -> Result<PathBuf, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.readlink(&relative).await
    }

    async fn symlink(&self, target: &Path, link: &Path) -> Result<(), FSError> {
        let (fs, relative_link) = self.resolve_fs(link)?;
        // Target is passed as-is; backend interprets relative to link's directory
        fs.symlink(target, &relative_link).await
    }

    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Vec<u8>, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.get_xattr(&relative, name).await
    }

    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.set_xattr(&relative, name, value).await
    }

    async fn remove_xattr(&self, path: &Path, name: &str) -> Result<(), FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.remove_xattr(&relative, name).await
    }

    async fn list_xattr(&self, path: &Path) -> Result<Vec<String>, FSError> {
        let (fs, relative) = self.resolve_fs(path)?;
        fs.list_xattr(&relative).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyFS;

    #[async_trait]
    impl ReadFS for DummyFS {
        async fn read(&self, _path: &Path, _offset: i64, _size: i64) -> Result<Vec<u8>, FSError> {
            Ok(vec![])
        }

        async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
            Ok(FileMetadata::file(path.to_path_buf(), 0))
        }
    }

    #[async_trait]
    impl WriteFS for DummyFS {
        async fn create(&self, _path: &Path) -> Result<(), FSError> {
            Ok(())
        }

        async fn write(
            &self,
            _path: &Path,
            data: &[u8],
            _offset: i64,
            _flags: WriteFlags,
        ) -> Result<u64, FSError> {
            Ok(data.len() as u64)
        }
    }

    #[async_trait]
    impl DirFS for DummyFS {
        async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
            Ok(())
        }

        async fn read_dir(&self, _path: &Path) -> Result<Vec<FileMetadata>, FSError> {
            Ok(vec![])
        }

        async fn remove(&self, _path: &Path) -> Result<(), FSError> {
            Ok(())
        }

        async fn remove_all(&self, _path: &Path) -> Result<(), FSError> {
            Ok(())
        }

        async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
            Ok(())
        }
    }

    #[async_trait]
    impl ChmodFS for DummyFS {
        async fn chmod(&self, _path: &Path, _mode: u32) -> Result<(), FSError> {
            Ok(())
        }
    }

    #[async_trait]
    impl FileSystem for DummyFS {
        async fn truncate(&self, _path: &Path, _size: u64) -> Result<(), FSError> {
            Ok(())
        }

        async fn touch(&self, _path: &Path) -> Result<(), FSError> {
            Ok(())
        }
    }

    #[test]
    fn test_mount_and_resolve() {
        let vfs = MountableFS::new();
        let fs: Arc<dyn FileSystem> = Arc::new(DummyFS);

        vfs.mount(PathBuf::from("/memfs"), fs.clone()).unwrap();

        let (resolved_fs, relative) = vfs.resolve_fs(&PathBuf::from("/memfs/file.txt")).unwrap();
        assert!(Arc::ptr_eq(&resolved_fs, &fs));
        assert_eq!(relative, PathBuf::from("/file.txt"));
    }

    #[test]
    fn test_unmount() {
        let vfs = MountableFS::new();
        let fs = Arc::new(DummyFS);

        vfs.mount(PathBuf::from("/memfs"), fs.clone()).unwrap();
        vfs.unmount(&PathBuf::from("/memfs")).unwrap();

        assert!(vfs.resolve_fs(&PathBuf::from("/memfs/file.txt")).is_err());
    }

    #[test]
    fn test_list_mounts() {
        let vfs = MountableFS::new();
        let fs = Arc::new(DummyFS);

        vfs.mount(PathBuf::from("/memfs"), fs.clone()).unwrap();
        vfs.mount(PathBuf::from("/tmp"), fs.clone()).unwrap();

        let mounts = vfs.list_mounts();
        assert_eq!(mounts.len(), 2);
    }

    #[test]
    fn test_duplicate_mount() {
        let vfs = MountableFS::new();
        let fs = Arc::new(DummyFS);

        vfs.mount(PathBuf::from("/memfs"), fs.clone()).unwrap();
        let result = vfs.mount(PathBuf::from("/memfs"), fs);

        assert!(result.is_err());
    }

    #[test]
    fn test_mount_at_root_fails() {
        let vfs = MountableFS::new();
        let fs: Arc<dyn FileSystem> = Arc::new(DummyFS);
        let result = vfs.mount(PathBuf::from("/"), fs);
        assert!(result.is_err());
        assert!(matches!(result, Err(FSError::InvalidInput { .. })));
    }

    #[test]
    fn test_resolve_path_not_found() {
        let vfs = MountableFS::new();
        let result = vfs.resolve_fs(&PathBuf::from("/nonexistent/foo"));
        assert!(result.is_err());
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_delegation_read() {
        let vfs = MountableFS::new();
        let fs: Arc<dyn FileSystem> = Arc::new(DummyFS);
        vfs.mount(PathBuf::from("/m"), fs).unwrap();
        let data = vfs.read(Path::new("/m/file"), 0, -1).await.unwrap();
        assert!(data.is_empty());
    }

    #[tokio::test]
    async fn test_delegation_stat() {
        let vfs = MountableFS::new();
        let fs: Arc<dyn FileSystem> = Arc::new(DummyFS);
        vfs.mount(PathBuf::from("/m"), fs).unwrap();
        let meta = vfs.stat(Path::new("/m/file")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/file"));
        assert_eq!(meta.size, 0);
    }

    #[tokio::test]
    async fn test_delegation_read_dir() {
        let vfs = MountableFS::new();
        let fs: Arc<dyn FileSystem> = Arc::new(DummyFS);
        vfs.mount(PathBuf::from("/m"), fs).unwrap();
        let entries = vfs.read_dir(Path::new("/m")).await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_delegation_create_and_write() {
        let vfs = MountableFS::new();
        let fs: Arc<dyn FileSystem> = Arc::new(DummyFS);
        vfs.mount(PathBuf::from("/m"), fs).unwrap();
        vfs.create(Path::new("/m/f")).await.unwrap();
        let n = vfs
            .write(Path::new("/m/f"), b"hi", 0, WriteFlags::none())
            .await
            .unwrap();
        assert_eq!(n, 2);
    }

    #[tokio::test]
    async fn test_rename_cross_device_fails() {
        let vfs = MountableFS::new();
        let fs1: Arc<dyn FileSystem> = Arc::new(DummyFS);
        let fs2: Arc<dyn FileSystem> = Arc::new(DummyFS);
        vfs.mount(PathBuf::from("/a"), fs1).unwrap();
        vfs.mount(PathBuf::from("/b"), fs2).unwrap();
        vfs.create(Path::new("/a/f")).await.unwrap();
        let result = vfs.rename(Path::new("/a/f"), Path::new("/b/g")).await;
        assert!(matches!(result, Err(FSError::CrossDeviceLink)));
    }
}
