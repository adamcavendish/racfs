//! Core types for RACFS (Remote Agent Communication File System).
//!
//! This crate provides the foundational traits, errors, and types for
//! implementing virtual filesystems with POSIX-compatible semantics.
//!
//! # Overview
//!
//! - **[`FileSystem`]** — The core async trait that all backends (MemFS,
//!   S3FS, LocalFS, etc.) implement. Methods include `create`, `mkdir`, `read`, `write`, `read_dir`,
//!   `stat`, `rename`, `chmod`, and optional `truncate` / `touch`.
//! - **[`FSError`]** — Error type for filesystem operations; used as
//!   `Result<T, FSError>` for all filesystem methods.
//! - **[`FileMetadata`]** — File/directory metadata (path, size, mode,
//!   timestamps). Helpers: [`FileMetadata::file`](metadata::FileMetadata::file),
//!   [`FileMetadata::directory`](metadata::FileMetadata::directory).
//! - **[`WriteFlags`]** / **[`OpenFlags`]** — Flags for write
//!   and open operations.
//!
//! # Caching
//!
//! - **[`Cache`]** — Trait for pluggable key-value caches (metadata/content).
//!   **[`CacheStats`]** provides hit rate and eviction counts; **[`HashMapCache`]**
//!   is a simple unbounded in-memory implementation; **[`FoyerCache`]** is the built-in
//!   bounded cache (foyer) with configurable eviction and optional hybrid (memory + disk).
//!
//! # Extension traits
//!
//! Optional traits (implement only if your backend supports them): [`HandleFS`](filesystem::HandleFS),
//! [`Symlinker`](filesystem::Symlinker), [`Toucher`](filesystem::Toucher), [`XattrFS`](filesystem::XattrFS),
//! [`Linker`](filesystem::Linker), [`RealPathFS`](filesystem::RealPathFS).
//!
//! # Example
//!
//! ```ignore
//! use racfs_core::{FileSystem, FileMetadata, FSError};
//! use std::path::Path;
//! use async_trait::async_trait;
//!
//! struct MyFS;
//! #[async_trait]
//! impl FileSystem for MyFS {
//!     async fn create(&self, _path: &Path) -> Result<(), FSError> { Err(FSError::ReadOnly) }
//!     async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> { Err(FSError::ReadOnly) }
//!     async fn remove(&self, _path: &Path) -> Result<(), FSError> { Err(FSError::ReadOnly) }
//!     async fn remove_all(&self, _path: &Path) -> Result<(), FSError> { Err(FSError::ReadOnly) }
//!     async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
//!         if path == Path::new("/hello") { Ok(b"hi".to_vec()) } else { Err(FSError::NotFound { path: path.into() }) }
//!     }
//!     async fn write(&self, _: &Path, _: &[u8], _: i64, _: racfs_core::WriteFlags) -> Result<u64, FSError> { Err(FSError::ReadOnly) }
//!     async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
//!         if path == Path::new("/") { Ok(vec![FileMetadata::file(std::path::PathBuf::from("/hello"), 2)]) }
//!         else { Err(FSError::NotFound { path: path.to_path_buf() }) }
//!     }
//!     async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
//!         if path == Path::new("/") { Ok(FileMetadata::directory(path.to_path_buf())) }
//!         else if path == Path::new("/hello") { Ok(FileMetadata::file(path.to_path_buf(), 2)) }
//!         else { Err(FSError::NotFound { path: path.to_path_buf() }) }
//!     }
//!     async fn rename(&self, _: &Path, _: &Path) -> Result<(), FSError> { Err(FSError::ReadOnly) }
//!     async fn chmod(&self, _: &Path, _: u32) -> Result<(), FSError> { Err(FSError::ReadOnly) }
//! }
//! ```
//!
//! See `docs/plugin-development.md` and the `custom_plugin` example in the racfs-vfs crate.

pub mod cache;
pub mod compression;
pub mod error;
pub mod file_handle;
pub mod filesystem;
pub mod flags;
pub mod metadata;

pub use cache::{Cache, CacheStats, FoyerCache, HashMapCache};
pub use compression::{Compression, CompressionLevel, ZstdCompression};
pub use error::FSError;
pub use file_handle::{FileHandle, HandleId};
pub use filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};
pub use flags::{OpenFlags, WriteFlags};
pub use metadata::FileMetadata;

#[cfg(test)]
mod proptest_tests;
