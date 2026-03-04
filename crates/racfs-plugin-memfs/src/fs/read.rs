use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::MemFS;

#[async_trait]
impl ReadFS for MemFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let entry = self.get_entry(path)?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        if entry.is_symlink
            && let Some(target) = &entry.symlink_target
        {
            return self.read(target, offset, size).await;
        }

        let data = &entry.data;
        let start = offset.max(0) as usize;
        let end = if size < 0 {
            data.len()
        } else {
            (offset + size).min(data.len() as i64) as usize
        };

        if start >= data.len() {
            return Ok(Vec::new());
        }

        self.inc_op("read");
        Ok(data[start..end].to_vec())
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let entry = self.get_entry(path)?;
        self.inc_op("stat");
        Ok(entry.metadata.clone())
    }
}
