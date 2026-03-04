use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};
use rusqlite::params;

use super::SqlFS;

#[async_trait]
impl ReadFS for SqlFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let path_str = SqlFS::path_to_str(path)?;

        if self.is_dir(&path_str)? {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT data FROM files WHERE path = ?1")
            .map_err(|e| FSError::Io {
                message: format!("Failed to prepare read query: {}", e),
            })?;

        let data: Vec<u8> = stmt
            .query_row(params![path_str], |row| {
                row.get::<_, Option<Vec<u8>>>(0)
                    .map(|opt| opt.unwrap_or_default())
            })
            .map_err(|e| FSError::Io {
                message: format!("Failed to read data: {}", e),
            })?;

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
        let path_str = SqlFS::path_to_str(path)?;
        self.get_metadata(&path_str)
    }
}
