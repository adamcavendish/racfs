//! RACFS FUSE - Mount RACFS as a native filesystem
//!
//! This crate provides FUSE (Filesystem in Userspace) support for RACFS,
//! allowing any RACFS server to be mounted as a local filesystem.
//!
//! # Operations
//!
//! - **mount()** uses a blocking adapter that implements fuser's sync `Filesystem` by
//!   delegating to `RacfsAsyncFs` (AsyncFilesystemCompat), giving full read-write.
//! - **mount_async()** uses fuser's `TokioAdapter(RacfsAsyncFs)` for read-only (lookup, getattr,
//!   read, readdir) without blocking the thread.
//!
//! # Features
//!
//! - **libfuse3**: Enable the `libfuse3` feature to link against libfuse3 for mount/umount (e.g.
//!   `cargo build -p racfs-fuse --features libfuse3`). Without it, on Linux, fuser uses its
//!   pure-Rust mount.
//!
//! # Resilience
//!
//! - **Client**: Retry with exponential backoff on connection/timeout; circuit breaker
//!   opens after repeated failures and stops calling the backend until cooldown.
//! - **Cache**: TTL cache for metadata and readdir; when the backend is unreachable,
//!   read-only operations may serve stale cache (graceful degradation).
//!
//! # Example
//!
//! ```ignore
//! use racfs_fuse::mount;
//! use std::path::PathBuf;
//!
//! mount("http://localhost:8080", PathBuf::from("/tmp/racfs"))?;
//! ```

mod async_fs;
mod blocking_adapter;
mod cache;
mod error;
mod inode_manager;

pub mod advanced;

pub use async_fs::{AsyncFilesystemCompat, RacfsAsyncFs};
pub use cache::{
    DEFAULT_TTL_SECS, DEFAULT_WRITE_BACK_FLUSH_SIZE, DEFAULT_WRITE_BACK_MAX_CHUNK, FuseCache,
};
pub use error::Error;
pub use inode_manager::InodeManager;

/// Re-export for building custom mounts. We use only fuser's AsyncFilesystem via TokioAdapter.
pub use fuser::experimental::TokioAdapter;

use std::path::PathBuf;
use tracing::info;

/// Mount a RACFS server at the specified mount point (read-write).
///
/// Uses a blocking adapter that implements fuser's `Filesystem` by delegating to
/// `RacfsAsyncFs`; all logic remains in the async implementation. For read-only async
/// mounting without blocking, use [`mount_async`].
pub fn mount(server_url: &str, mount_point: PathBuf) -> Result<(), Error> {
    info!("Mounting RACFS from {} at {:?}", server_url, mount_point);

    let adapter = blocking_adapter::BlockingAdapter::new(server_url)?;

    let mut config = fuser::Config::default();
    config
        .mount_options
        .push(fuser::MountOption::FSName("racfs".to_string()));
    config.mount_options.push(fuser::MountOption::AutoUnmount);

    fuser::mount2(adapter, &mount_point, &config).map_err(|e| Error::MountFailed {
        path: mount_point.display().to_string(),
        message: e.to_string(),
    })?;

    Ok(())
}

/// Mount multiple RACFS servers at the given mount points in a single process.
///
/// Spawns one thread per (server_url, mount_point); each thread runs a blocking
/// FUSE session. The function blocks until **all** mounts are unmounted (e.g. via
/// fusermount -u or umount on each mount point). Order of unmount does not matter.
///
/// # Example
///
/// ```ignore
/// use racfs_fuse::mount_multi;
/// use std::path::PathBuf;
///
/// mount_multi([
///     ("http://localhost:8080", PathBuf::from("/tmp/racfs1")),
///     ("http://localhost:8081", PathBuf::from("/tmp/racfs2")),
/// ])?;
/// ```
pub fn mount_multi(
    mounts: impl IntoIterator<Item = (impl AsRef<str>, PathBuf)>,
) -> Result<(), Error> {
    let mounts: Vec<_> = mounts
        .into_iter()
        .map(|(u, p)| (u.as_ref().to_string(), p))
        .collect();

    if mounts.is_empty() {
        return Ok(());
    }

    let mut handles = Vec::with_capacity(mounts.len());
    for (server_url, mount_point) in mounts {
        let url = server_url.clone();
        let path_for_thread = mount_point.clone();
        let h = std::thread::spawn(move || mount(&url, path_for_thread));
        handles.push((mount_point, h));
    }

    let mut first_error = Ok(());
    for (path, h) in handles {
        match h.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                if first_error.is_ok() {
                    first_error = Err(e);
                }
            }
            Err(_) => {
                if first_error.is_ok() {
                    first_error = Err(Error::MountFailed {
                        path: path.display().to_string(),
                        message: "mount thread panicked".to_string(),
                    });
                }
            }
        }
    }

    first_error
}

/// Mount using fuser's experimental async API (TokioAdapter), read-only.
///
/// Lookup, getattr, read, readdir run natively async. For full read-write use [`mount`].
pub fn mount_async(server_url: &str, mount_point: PathBuf) -> Result<(), Error> {
    info!(
        "Mounting RACFS (async read-only) from {} at {:?}",
        server_url, mount_point
    );

    let fs = RacfsAsyncFs::new(server_url)?;
    let adapter = fuser::experimental::TokioAdapter::new(fs);

    let mut config = fuser::Config::default();
    config
        .mount_options
        .push(fuser::MountOption::FSName("racfs".to_string()));
    config.mount_options.push(fuser::MountOption::AutoUnmount);
    config.mount_options.push(fuser::MountOption::RO);

    fuser::mount2(adapter, &mount_point, &config).map_err(|e| Error::MountFailed {
        path: mount_point.display().to_string(),
        message: e.to_string(),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_requires_valid_url() {
        // RacfsAsyncFs::new rejects invalid URL (e.g. empty) by returning error or succeeding with URL
        let r = RacfsAsyncFs::new("http://127.0.0.1:0");
        assert!(r.is_ok());
    }

    #[test]
    fn test_default_ttl_constants() {
        assert_eq!(cache::DEFAULT_TTL_SECS, 1);
        const _: () = assert!(cache::DEFAULT_WRITE_BACK_FLUSH_SIZE > 0);
        const _: () = assert!(cache::DEFAULT_WRITE_BACK_MAX_CHUNK > 0);
    }

    #[test]
    fn test_mount_multi_empty_returns_ok() {
        use std::path::PathBuf;
        let r: Result<(), _> = super::mount_multi([] as [(String, PathBuf); 0]);
        assert!(r.is_ok());
    }
}
