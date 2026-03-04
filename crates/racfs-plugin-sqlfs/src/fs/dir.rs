use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use racfs_core::{
    error::FSError,
    filesystem::DirFS,
    metadata::{FileMetadata, S_IFDIR, S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH},
};
use rusqlite::params;

use super::SqlFS;

#[async_trait]
impl DirFS for SqlFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let path_str = SqlFS::path_to_str(path)?;
        self.ensure_parent_exists(&path_str)?;

        if self.exists(&path_str)? {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let now = Utc::now().timestamp_millis();
        let mode = S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;

        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO files (path, data, is_dir, mode, created_at, modified_at, accessed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![&path_str, &Vec::<u8>::new(), &true, &mode, &now, &now, &now],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to create directory: {}", e),
        })?;

        tracing::debug!(path = %path.display(), "created directory");
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let path_str = SqlFS::path_to_str(path)?;

        if !self.is_dir(&path_str)? {
            return Err(FSError::NotADirectory {
                path: path.to_path_buf(),
            });
        }

        let conn = self.conn.lock();

        let pattern = if path_str == "/" {
            "/%".to_string()
        } else {
            format!("{}/%", path_str)
        };

        let mut stmt = conn
            .prepare(
                r#"
                SELECT path, data, is_dir, mode, created_at, modified_at, accessed_at
                FROM files
                WHERE path LIKE ?1
                "#,
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to prepare read_dir query: {}", e),
            })?;

        let rows = stmt
            .query_map(params![&pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            })
            .map_err(|e| FSError::Io {
                message: format!("Failed to query read_dir: {}", e),
            })?;

        let mut entries = Vec::new();
        for row_result in rows {
            let (path, data, _is_dir, mode, created, modified, accessed) =
                row_result.map_err(|e| FSError::Io {
                    message: format!("Failed to read row: {}", e),
                })?;

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
                    mode,
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
        let path_str = SqlFS::path_to_str(path)?;

        if !self.exists(&path_str)? {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        if self.is_dir(&path_str)? {
            let conn = self.conn.lock();
            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM files WHERE path LIKE ?1 || '/%' LIMIT 1")
                .map_err(|e| FSError::Io {
                    message: format!("Failed to prepare empty check query: {}", e),
                })?;

            let count: i64 = stmt
                .query_row(params![path_str], |row| row.get(0))
                .map_err(|e| FSError::Io {
                    message: format!("Failed to check if directory is empty: {}", e),
                })?;

            if count > 0 {
                return Err(FSError::DirectoryNotEmpty);
            }
        }

        let conn = self.conn.lock();
        conn.execute("DELETE FROM files WHERE path = ?1", params![path_str])
            .map_err(|e| FSError::Io {
                message: format!("Failed to remove: {}", e),
            })?;

        tracing::debug!(path = %path.display(), "removed");
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let path_str = SqlFS::path_to_str(path)?;

        if !self.exists(&path_str)? {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM files WHERE path = ?1 OR path LIKE ?1 || '/%'",
            params![path_str],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to remove all: {}", e),
        })?;

        tracing::debug!(path = %path.display(), "removed all");
        Ok(())
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let old_str = SqlFS::path_to_str(old_path)?;
        let new_str = SqlFS::path_to_str(new_path)?;

        if !self.exists(&old_str)? {
            return Err(FSError::NotFound {
                path: old_path.to_path_buf(),
            });
        }

        if self.exists(&new_str)? {
            return Err(FSError::AlreadyExists {
                path: new_path.to_path_buf(),
            });
        }

        if self.is_dir(&old_str)? {
            let conn = self.conn.lock();

            let pattern = format!("{}/%", old_str);
            conn.execute(
                r#"
                UPDATE files
                SET path = ?1 || SUBSTR(path, ?2 + 1)
                WHERE path LIKE ?3
                "#,
                params![&new_str, old_str.len(), &pattern],
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to move directory children: {}", e),
            })?;

            conn.execute(
                r#"
                UPDATE files
                SET path = ?1
                WHERE path = ?2
                "#,
                params![&new_str, &old_str],
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to move directory: {}", e),
            })?;
        } else {
            let conn = self.conn.lock();
            conn.execute(
                r#"
                UPDATE files
                SET path = ?1
                WHERE path = ?2
                "#,
                params![&new_str, &old_str],
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to rename: {}", e),
            })?;
        }

        tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed");
        Ok(())
    }
}
