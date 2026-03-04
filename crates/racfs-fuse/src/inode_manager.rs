//! Inode management for RACFS FUSE
//!
//! Maps FUSE inodes (u64) to filesystem paths.

use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Root inode number (always 1 in FUSE)
pub const ROOT_INODE: u64 = 1;

/// Manages mapping between inodes and paths
pub struct InodeManager {
    /// Map from inode to path
    inode_to_path: DashMap<u64, PathBuf>,
    /// Map from path to inode
    path_to_inode: DashMap<PathBuf, u64>,
    /// Next available inode number
    next_inode: AtomicU64,
}

impl InodeManager {
    /// Create a new inode manager
    pub fn new() -> Self {
        let manager = Self {
            inode_to_path: DashMap::new(),
            path_to_inode: DashMap::new(),
            next_inode: AtomicU64::new(ROOT_INODE + 1),
        };

        // Register root inode
        manager.inode_to_path.insert(ROOT_INODE, PathBuf::from("/"));
        manager.path_to_inode.insert(PathBuf::from("/"), ROOT_INODE);

        manager
    }

    /// Allocate an inode for a path, or return existing inode if already allocated
    pub fn allocate(&self, path: &Path) -> u64 {
        // Check if path already has an inode
        if let Some(entry) = self.path_to_inode.get(path) {
            return *entry;
        }

        // Allocate new inode
        let inode = self.next_inode.fetch_add(1, Ordering::SeqCst);
        self.inode_to_path.insert(inode, path.to_path_buf());
        self.path_to_inode.insert(path.to_path_buf(), inode);

        inode
    }

    /// Get path for an inode
    pub fn get_path(&self, inode: u64) -> Option<PathBuf> {
        self.inode_to_path.get(&inode).map(|entry| entry.clone())
    }

    /// Get inode for a path
    pub fn get_inode(&self, path: &Path) -> Option<u64> {
        self.path_to_inode.get(path).map(|entry| *entry)
    }

    /// Remove an inode
    pub fn remove(&self, inode: u64) {
        if let Some((_, path)) = self.inode_to_path.remove(&inode) {
            self.path_to_inode.remove(&path);
        }
    }

    /// Remove by path
    pub fn remove_path(&self, path: &Path) {
        if let Some((_, inode)) = self.path_to_inode.remove(path) {
            self.inode_to_path.remove(&inode);
        }
    }
}

impl Default for InodeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_inode() {
        let manager = InodeManager::new();
        assert_eq!(manager.get_path(ROOT_INODE), Some(PathBuf::from("/")));
        assert_eq!(manager.get_inode(Path::new("/")), Some(ROOT_INODE));
    }

    #[test]
    fn test_allocate_inode() {
        let manager = InodeManager::new();
        let path = Path::new("/test.txt");

        let inode1 = manager.allocate(path);
        assert!(inode1 > ROOT_INODE);

        // Allocating same path should return same inode
        let inode2 = manager.allocate(path);
        assert_eq!(inode1, inode2);

        // Should be able to retrieve path
        assert_eq!(manager.get_path(inode1), Some(path.to_path_buf()));
        assert_eq!(manager.get_inode(path), Some(inode1));
    }

    #[test]
    fn test_remove_inode() {
        let manager = InodeManager::new();
        let path = Path::new("/test.txt");

        let inode = manager.allocate(path);
        assert!(manager.get_path(inode).is_some());

        manager.remove(inode);
        assert!(manager.get_path(inode).is_none());
        assert!(manager.get_inode(path).is_none());
    }

    #[test]
    fn test_remove_path() {
        let manager = InodeManager::new();
        let path = Path::new("/test.txt");

        let inode = manager.allocate(path);
        assert!(manager.get_inode(path).is_some());

        manager.remove_path(path);
        assert!(manager.get_path(inode).is_none());
        assert!(manager.get_inode(path).is_none());
    }

    #[test]
    fn test_multiple_paths() {
        let manager = InodeManager::new();

        let path1 = Path::new("/file1.txt");
        let path2 = Path::new("/file2.txt");
        let path3 = Path::new("/dir/file3.txt");

        let inode1 = manager.allocate(path1);
        let inode2 = manager.allocate(path2);
        let inode3 = manager.allocate(path3);

        // All inodes should be unique
        assert_ne!(inode1, inode2);
        assert_ne!(inode2, inode3);
        assert_ne!(inode1, inode3);

        // All paths should be retrievable
        assert_eq!(manager.get_path(inode1), Some(path1.to_path_buf()));
        assert_eq!(manager.get_path(inode2), Some(path2.to_path_buf()));
        assert_eq!(manager.get_path(inode3), Some(path3.to_path_buf()));
    }

    #[test]
    fn test_default_equals_new() {
        let default_manager = InodeManager::default();
        let new_manager = InodeManager::new();
        assert_eq!(
            default_manager.get_path(ROOT_INODE),
            new_manager.get_path(ROOT_INODE)
        );
        assert_eq!(default_manager.get_inode(Path::new("/")), Some(ROOT_INODE));
    }
}
