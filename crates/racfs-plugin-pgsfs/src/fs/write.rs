use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{
    error::FSError,
    filesystem::WriteFS,
    flags::WriteFlags,
    metadata::{S_IFREG, S_IRGRP, S_IROTH, S_IRWXU},
};
use sqlx::Row;

use super::PgsFS;

#[async_trait]
impl WriteFS for PgsFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let path_str = PgsFS::path_to_str(path)?;
        self.ensure_parent_exists(&path_str).await?;

        if self.exists(&path_str).await? {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let now = Utc::now().timestamp_millis();
        let mode = S_IFREG | S_IRWXU | S_IRGRP | S_IROTH;

        sqlx::query(
            r#"
            INSERT INTO files (path, data, is_dir, mode, created_at, modified_at, accessed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&path_str)
        .bind(&Vec::<u8>::new())
        .bind(&false)
        .bind(&(mode as i32))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to create file: {}", e),
        })?;

        tracing::debug!(path = %path.display(), "created file");
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let path_str = PgsFS::path_to_str(path)?;

        if self.is_dir(&path_str).await? {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        if data.len() > self.config.max_file_size {
            return Err(FSError::StorageFull);
        }

        let row = sqlx::query("SELECT data FROM files WHERE path = $1")
            .bind(&path_str)
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to read for write: {}", e),
            })?;

        let existing_data: Vec<u8> = row
            .and_then(|r| r.get::<Option<Vec<u8>>, _>("data"))
            .unwrap_or_default();

        let is_truncate = flags.contains(WriteFlags::from_bits_truncate(0x0020));
        let new_data = if flags.contains_append() || offset as usize >= existing_data.len() {
            let mut result = existing_data.clone();
            let end = offset.max(0) as usize;
            if end > result.len() {
                result.resize(end, 0);
            }
            result.extend_from_slice(data);
            result
        } else if is_truncate || offset <= 0 {
            data.to_vec()
        } else {
            let start = offset as usize;
            let mut result = existing_data.clone();
            if start > result.len() {
                result.resize(start, 0);
            }
            if start + data.len() > result.len() {
                result.resize(start + data.len(), 0);
            }
            result[start..start + data.len()].copy_from_slice(data);
            result
        };

        let now = Utc::now().timestamp_millis();
        sqlx::query(
            r#"
            UPDATE files
            SET data = $1, modified_at = $2, accessed_at = $3
            WHERE path = $4
            "#,
        )
        .bind(&new_data)
        .bind(&now)
        .bind(&now)
        .bind(&path_str)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to write: {}", e),
        })?;

        tracing::debug!(path = %path.display(), bytes = data.len(), "wrote");
        Ok(data.len() as u64)
    }
}
