use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{
    error::FSError,
    filesystem::WriteFS,
    flags::WriteFlags,
    metadata::{S_IFREG, S_IRGRP, S_IROTH, S_IRWXU},
};
use rusqlite::{OptionalExtension, params};

use super::SqlFS;

#[async_trait]
impl WriteFS for SqlFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let path_str = SqlFS::path_to_str(path)?;
        self.ensure_parent_exists(&path_str)?;

        if self.exists(&path_str)? {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let now = Utc::now().timestamp_millis();
        let mode = S_IFREG | S_IRWXU | S_IRGRP | S_IROTH;

        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO files (path, data, is_dir, mode, created_at, modified_at, accessed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                &path_str,
                &Vec::<u8>::new(),
                &false,
                &mode,
                &now,
                &now,
                &now
            ],
        )
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
        let path_str = SqlFS::path_to_str(path)?;

        if self.is_dir(&path_str)? {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        if data.len() > self.config.max_file_size {
            return Err(FSError::StorageFull);
        }

        let conn = self.conn.lock();

        let existing_data: Vec<u8> = {
            let mut stmt = conn
                .prepare("SELECT data FROM files WHERE path = ?1")
                .map_err(|e| FSError::Io {
                    message: format!("Failed to prepare write read query: {}", e),
                })?;

            stmt.query_row(params![path_str], |row| {
                row.get::<_, Option<Vec<u8>>>(0)
                    .map(|opt| opt.unwrap_or_default())
            })
            .optional()
            .unwrap_or_default()
            .unwrap_or_default()
        };

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
        conn.execute(
            r#"
            UPDATE files
            SET data = ?1, modified_at = ?2, accessed_at = ?3
            WHERE path = ?4
            "#,
            params![&new_data, &now, &now, &path_str],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to write: {}", e),
        })?;

        tracing::debug!(path = %path.display(), bytes = data.len(), "wrote");
        Ok(data.len() as u64)
    }
}
