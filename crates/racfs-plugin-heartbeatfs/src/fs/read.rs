use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::HeartbeatFS;

#[async_trait]
impl ReadFS for HeartbeatFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        if !self.is_valid_path(path) {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let data = self.get_entry(path)?;

        let start = offset.max(0) as usize;
        let end = if size < 0 {
            data.len()
        } else {
            (offset + size).min(data.len() as i64) as usize
        };

        if start >= data.len() {
            return Ok(Vec::new());
        }

        Ok(data[start..end].to_vec())
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        if path == Path::new("/") {
            return Ok(FileMetadata::directory(PathBuf::from("/")));
        }

        if !self.is_valid_path(path) {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let data = self.get_entry(path)?;
        Ok(FileMetadata::file(path.to_path_buf(), data.len() as u64))
    }
}
