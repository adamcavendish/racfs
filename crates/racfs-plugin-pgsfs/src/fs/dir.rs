use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::DateTime;
use racfs_core::{
    error::FSError,
    filesystem::DirFS,
    metadata::{FileMetadata, S_IFDIR, S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH},
};
use sqlx::Row;

use super::PgsFS;

#[async_trait]
impl DirFS for PgsFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let path_str = PgsFS::path_to_str(path)?;
        self.ensure_parent_exists(&path_str).await?;

        if self.exists(&path_str).await? {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mode = S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;

        sqlx::query(
            r#"
            INSERT INTO files (path, data, is_dir, mode, created_at, modified_at, accessed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&path_str)
        .bind(&Vec::<u8>::new())
        .bind(&true)
        .bind(&(mode as i32))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to create directory: {}", e),
        })?;

        tracing::debug!(path = %path.display(), "created directory");
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let path_str = PgsFS::path_to_str(path)?;

        if !self.is_dir(&path_str).await? {
            return Err(FSError::NotADirectory {
                path: path.to_path_buf(),
            });
        }

        let pattern = if path_str == "/" {
            "/%".to_string()
        } else {
            format!("{}/%", path_str)
        };

        let rows = sqlx::query(
            r#"
            SELECT path, data, is_dir, mode, created_at, modified_at, accessed_at
            FROM files
            WHERE path LIKE $1
            "#,
        )
        .bind(&pattern)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to query read_dir: {}", e),
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let path: String = row.get("path");
            let data: Option<Vec<u8>> = row.get("data");
            let _is_dir: bool = row.get("is_dir");
            let mode: i32 = row.get("mode");
            let created: Option<i64> = row.get("created_at");
            let modified: Option<i64> = row.get("modified_at");
            let accessed: Option<i64> = row.get("accessed_at");

            let full_path = PathBuf::from(&path);

            let relative = if path_str == "/" {
                full_path.clone()
            } else {
                full_path
                    .strip_prefix(&path_str)
                    .unwrap_or(&full_path)
                    .to_path_buf()
            };

            let relative_str = relative.to_string_lossy();
            let is_direct_child = if path_str == "/" {
                relative_str.matches('/').count() == 1
            } else {
                !relative_str.contains('/')
            };

            if is_direct_child {
                let size = data.unwrap_or_default().len() as u64;

                entries.push(FileMetadata {
                    path: full_path,
                    size,
                    mode: mode as u32,
                    created: created.and_then(DateTime::from_timestamp_millis),
                    modified: modified.and_then(DateTime::from_timestamp_millis),
                    accessed: accessed.and_then(DateTime::from_timestamp_millis),
                    is_symlink: false,
                    symlink_target: None,
                });
            }
        }

        Ok(entries)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let path_str = PgsFS::path_to_str(path)?;

        if !self.exists(&path_str).await? {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        if self.is_dir(&path_str).await? {
            let pattern = format!("{}/%", path_str);
            let row = sqlx::query("SELECT COUNT(*) as count FROM files WHERE path LIKE $1")
                .bind(&pattern)
                .fetch_one(self.pool.as_ref())
                .await
                .map_err(|e| FSError::Io {
                    message: format!("Failed to check if directory is empty: {}", e),
                })?;

            let count: i64 = row.get("count");

            if count > 0 {
                return Err(FSError::DirectoryNotEmpty);
            }
        }

        sqlx::query("DELETE FROM files WHERE path = $1")
            .bind(&path_str)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to remove: {}", e),
            })?;

        tracing::debug!(path = %path.display(), "removed");
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let path_str = PgsFS::path_to_str(path)?;

        if !self.exists(&path_str).await? {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let pattern = format!("{}/%", path_str);
        sqlx::query("DELETE FROM files WHERE path = $1 OR path LIKE $2")
            .bind(&path_str)
            .bind(&pattern)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to remove all: {}", e),
            })?;

        tracing::debug!(path = %path.display(), "removed all");
        Ok(())
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let old_str = PgsFS::path_to_str(old_path)?;
        let new_str = PgsFS::path_to_str(new_path)?;

        if !self.exists(&old_str).await? {
            return Err(FSError::NotFound {
                path: old_path.to_path_buf(),
            });
        }

        if self.exists(&new_str).await? {
            return Err(FSError::AlreadyExists {
                path: new_path.to_path_buf(),
            });
        }

        if self.is_dir(&old_str).await? {
            let pattern = format!("{}/%", old_str);

            let rows = sqlx::query("SELECT path FROM files WHERE path LIKE $1")
                .bind(&pattern)
                .fetch_all(self.pool.as_ref())
                .await
                .map_err(|e| FSError::Io {
                    message: format!("Failed to query children: {}", e),
                })?;

            for row in rows {
                let child_path: String = row.get("path");
                let new_child_path = child_path.replacen(&old_str, &new_str, 1);

                sqlx::query("UPDATE files SET path = $1 WHERE path = $2")
                    .bind(&new_child_path)
                    .bind(&child_path)
                    .execute(self.pool.as_ref())
                    .await
                    .map_err(|e| FSError::Io {
                        message: format!("Failed to move child: {}", e),
                    })?;
            }

            sqlx::query("UPDATE files SET path = $1 WHERE path = $2")
                .bind(&new_str)
                .bind(&old_str)
                .execute(self.pool.as_ref())
                .await
                .map_err(|e| FSError::Io {
                    message: format!("Failed to move directory: {}", e),
                })?;
        } else {
            sqlx::query("UPDATE files SET path = $1 WHERE path = $2")
                .bind(&new_str)
                .bind(&old_str)
                .execute(self.pool.as_ref())
                .await
                .map_err(|e| FSError::Io {
                    message: format!("Failed to rename: {}", e),
                })?;
        }

        tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed");
        Ok(())
    }
}
