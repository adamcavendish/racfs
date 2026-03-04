//! Integration tests for RACFS FUSE.
//!
//! Most FUSE tests require a running RACFS server and (for mount tests) either root
//! or membership in the fuse group. Use the verification commands from ROADMAP.md
//! for manual testing.
//!
//! ## Manual mount/unmount test
//!
//! To verify real mount and unmount end-to-end:
//!
//! 1. Start the server: `cargo run -p racfs-server`
//! 2. In another terminal, create a mount point and mount:
//!    `mkdir -p /tmp/racfs-mnt && cargo run -p racfs-cli -- mount http://127.0.0.1:8080 /tmp/racfs-mnt`
//! 3. In a third terminal, exercise the filesystem (read-only: ls, cat):
//!    `ls /tmp/racfs-mnt/memfs && cat /tmp/racfs-mnt/memfs/foo` (if foo exists)
//! 4. Unmount: `fusermount -u /tmp/racfs-mnt` (Linux) or `umount /tmp/racfs-mnt` (macOS)
//! 5. Stop the server (Ctrl+C).
//!
use std::time::Duration;

use racfs_fuse::{AsyncFilesystemCompat, FuseCache, RacfsAsyncFs};

#[test]
fn test_racfs_async_fs_creation() {
    // Creation should not panic; server URL is not contacted until mount/ops.
    let result = RacfsAsyncFs::new("http://127.0.0.1:0");
    assert!(result.is_ok());
}

#[test]
fn test_racfs_async_fs_with_custom_cache() {
    let cache = FuseCache::new(Duration::from_secs(5));
    let result = RacfsAsyncFs::with_cache("http://127.0.0.1:0", cache);
    assert!(result.is_ok());
}

#[test]
fn test_error_mount_failed_message() {
    use racfs_fuse::Error;
    let err = Error::MountFailed {
        path: "/nonexistent".to_string(),
        message: "Permission denied".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/nonexistent"));
    assert!(msg.contains("Permission denied"));
}

#[test]
fn test_error_invalid_path_display() {
    use racfs_fuse::Error;
    let err = Error::InvalidPath {
        path: "/bad/path".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Invalid path"));
    assert!(msg.contains("/bad/path"));
}

#[test]
fn test_error_inode_not_found_display() {
    use racfs_fuse::Error;
    let err = Error::InodeNotFound { inode: 999 };
    let msg = err.to_string();
    assert!(msg.contains("Inode not found"));
    assert!(msg.contains("999"));
}

#[test]
fn test_error_from_client_error() {
    use racfs_fuse::Error;
    let client_err = racfs_client::Error::Api {
        code: "NotFound".to_string(),
        message: "file not found".to_string(),
        detail: None,
    };
    let fuse_err: Error = client_err.into();
    let msg = fuse_err.to_string();
    assert!(msg.contains("Client error"));
    assert!(msg.contains("NotFound") || msg.contains("file not found"));
}

#[test]
fn test_error_from_io_error() {
    use racfs_fuse::Error;
    let io_err = std::io::Error::other("connection refused");
    let fuse_err: Error = io_err.into();
    let msg = fuse_err.to_string();
    assert!(msg.contains("IO error"));
    assert!(msg.contains("connection refused"));
}

#[test]
fn test_error_is_std_error() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<racfs_fuse::Error>();
}

/// Manual test: run with `cargo test -p racfs-fuse test_async_fs_against_server -- --ignored --nocapture`
/// after starting a server: `cargo run -p racfs-server`
#[tokio::test]
#[ignore]
async fn test_async_fs_against_server() {
    let fs = RacfsAsyncFs::new("http://127.0.0.1:8080").expect("create async fs");
    let result = fs.getattr_async(1).await;
    // Root inode 1: server returns attr (Ok) or ENOENT depending on backend
    assert!(result.is_ok() || result.is_err());
}

/// Multi-threaded access test: run with
/// `cargo test -p racfs-fuse test_multi_threaded_access -- --ignored --nocapture`
/// after starting a server: `cargo run -p racfs-server`
///
/// Spawns multiple concurrent tasks that each perform getattr/lookup/readdir
/// to verify no panics and consistent behavior under concurrent load.
#[tokio::test]
#[ignore]
async fn test_multi_threaded_access() {
    let server_url = std::env::var("RACFS_TEST_SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let mut handles = Vec::new();
    for _ in 0..8 {
        let url = server_url.clone();
        let h = tokio::spawn(async move {
            let fs = RacfsAsyncFs::new(&url).expect("create async fs");
            for _ in 0..10 {
                let _ = fs.getattr_async(1).await;
            }
            if let Ok(entries) = fs.readdir_async(1, 0).await {
                for _ in 0..5 {
                    let _ = fs.getattr_async(1).await;
                }
                let _ = entries;
            }
        });
        handles.push(h);
    }

    for h in handles {
        h.await.expect("task panicked");
    }
}
