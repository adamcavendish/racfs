//! HelloFS filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use parking_lot::RwLock;
use racfs_core::{error::FSError, metadata::FileMetadata};

/// HelloFS - A simple static read-only demo filesystem.
///
/// Filesystem structure:
/// ```text
/// /
/// |-- hello      # Contains "Hello, World!"
/// |-- version    # Plugin version "1.0.0"
/// |-- readme.txt # Description of the plugin
/// ```
pub struct HelloFS {
    pub(super) files: Arc<RwLock<HashMap<PathBuf, FileEntry>>>,
}

#[derive(Clone)]
pub(super) struct FileEntry {
    pub(super) data: Vec<u8>,
    pub(super) metadata: FileMetadata,
}

impl HelloFS {
    /// Create a new HelloFS instance.
    pub fn new() -> Self {
        let mut files = HashMap::new();

        // Root directory
        let root_metadata = FileMetadata::directory(PathBuf::from("/"));
        files.insert(
            PathBuf::from("/"),
            FileEntry {
                data: Vec::new(),
                metadata: root_metadata,
            },
        );

        // /hello - Contains "Hello, World!"
        let hello_path = PathBuf::from("/hello");
        let hello_data = b"Hello, World!".to_vec();
        let hello_metadata = FileMetadata::file(hello_path.clone(), hello_data.len() as u64);
        files.insert(
            hello_path,
            FileEntry {
                data: hello_data,
                metadata: hello_metadata,
            },
        );

        // /version - Contains plugin version "1.0.0"
        let version_path = PathBuf::from("/version");
        let version_data = b"1.0.0".to_vec();
        let version_metadata = FileMetadata::file(version_path.clone(), version_data.len() as u64);
        files.insert(
            version_path,
            FileEntry {
                data: version_data,
                metadata: version_metadata,
            },
        );

        // /readme.txt - Description of the plugin
        let readme_path = PathBuf::from("/readme.txt");
        let readme_data = b"HelloFS - A simple static read-only demo filesystem for RACFS.\n\n\
This plugin demonstrates a minimal filesystem implementation with fixed content.\n\
All write operations will fail with a ReadOnly error.\n"
            .to_vec();
        let readme_metadata = FileMetadata::file(readme_path.clone(), readme_data.len() as u64);
        files.insert(
            readme_path,
            FileEntry {
                data: readme_data,
                metadata: readme_metadata,
            },
        );

        Self {
            files: Arc::new(RwLock::new(files)),
        }
    }

    pub(super) fn get_entry(&self, path: &Path) -> Result<FileEntry, FSError> {
        let files = self.files.read();
        files.get(path).cloned().ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })
    }
}

impl Default for HelloFS {
    fn default() -> Self {
        Self::new()
    }
}

// Override default touch to return ReadOnly.
#[async_trait]
impl racfs_core::FileSystem for HelloFS {
    async fn touch(&self, _path: &std::path::Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}
