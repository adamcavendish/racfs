use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};
use sqlx::Row;

use super::PgsFS;

#[async_trait]
impl ReadFS for PgsFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let path_str = PgsFS::path_to_str(path)?;

        if self.is_dir(&path_str).await? {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let row = sqlx::query("SELECT data FROM files WHERE path = $1")
            .bind(&path_str)
            .fetch_one(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to read data: {}", e),
            })?;

        let data: Vec<u8> = row.get::<Option<Vec<u8>>, _>("data").unwrap_or_default();

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
        let path_str = PgsFS::path_to_str(path)?;
        self.get_metadata(&path_str).await
    }
}
