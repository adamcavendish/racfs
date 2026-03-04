use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::HeartbeatFS;

#[async_trait]
impl DirFS for HeartbeatFS {
    async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        if path != Path::new("/") {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        Ok(vec![
            FileMetadata::file(PathBuf::from("/status"), 2),
            FileMetadata::file(PathBuf::from("/uptime"), 8),
            FileMetadata::file(PathBuf::from("/beats"), 8),
            FileMetadata::file(PathBuf::from("/last_beat"), 32),
            FileMetadata::file(PathBuf::from("/pulse"), 0),
        ])
    }

    async fn remove(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn remove_all(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}
