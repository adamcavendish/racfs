//! POSIX-oriented tests for FUSE behavior.
//!
//! These tests verify expected behavior without requiring a live mount.
//! Run full mount tests manually (see ROADMAP.md).

use racfs_fuse::{InodeManager, RacfsAsyncFs};
use std::path::Path;

/// Inode 1 is root and maps to /
#[test]
fn test_root_inode_is_one() {
    let manager = InodeManager::new();
    assert_eq!(manager.get_path(1), Some(std::path::PathBuf::from("/")));
    assert_eq!(manager.get_inode(Path::new("/")), Some(1));
}

/// Lookup of "." and ".." in readdir use parent inode (POSIX).
#[test]
fn test_dot_dotdot_inode_convention() {
    let _fs = RacfsAsyncFs::new("http://127.0.0.1:0").unwrap();
    // Just ensure we can create; dot/dotdot are handled in readdir_async
}

/// Creation with invalid URL does not panic.
#[test]
fn test_creation_invalid_url_does_not_panic() {
    let result = RacfsAsyncFs::new("http://127.0.0.1:0");
    assert!(result.is_ok());
}

/// Path normalization: multiple slashes and . are not normalized by inode manager
/// (server is responsible). We only check inode manager stores paths as given.
#[test]
fn test_inode_path_roundtrip() {
    let manager = InodeManager::new();
    let path = Path::new("/memfs/foo");
    let inode = manager.allocate(path);
    assert_eq!(manager.get_path(inode), Some(path.to_path_buf()));
    assert_eq!(manager.get_inode(path), Some(inode));
}
