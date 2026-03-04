use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::ServerInfoFS;

#[async_trait]
impl ReadFS for ServerInfoFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let entry = self.get_entry(path)?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let data = &entry.content;
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
        let entry = self.get_entry(path)?;
        Ok(entry.metadata.clone())
    }
}
