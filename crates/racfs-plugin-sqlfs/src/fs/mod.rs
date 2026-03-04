//! SQL-backed filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use racfs_core::{
    error::FSError,
    filesystem::{FileSystem, WriteFS},
    metadata::{FileMetadata, S_IFDIR, S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH},
};
use rusqlite::{Connection, OptionalExtension, params};

/// Configuration for the SQL filesystem.
#[derive(Debug, Clone)]
pub struct SqlConfig {
    /// SQLite connection string.
    pub database_url: String,
    /// Maximum file size in bytes.
    pub max_file_size: usize,
}

impl Default for SqlConfig {
    fn default() -> Self {
        Self {
            database_url: "file::memory:".to_string(),
            max_file_size: 5 * 1024 * 1024, // 5MB
        }
    }
}

/// SQL-backed filesystem using SQLite.
pub struct SqlFS {
    pub(super) config: SqlConfig,
    pub(super) conn: Arc<Mutex<Connection>>,
}

impl SqlFS {
    /// Create a new SQL filesystem with default configuration.
    pub fn new() -> Result<Self, FSError> {
        Self::with_config(SqlConfig::default())
    }

    /// Create a new SQL filesystem with the given configuration.
    pub fn with_config(config: SqlConfig) -> Result<Self, FSError> {
        let conn = Connection::open(&config.database_url).map_err(|e| FSError::Io {
            message: format!("Failed to open database: {}", e),
        })?;

        let conn = Arc::new(Mutex::new(conn));
        let fs = Self { config, conn };

        fs.initialize_schema()?;
        fs.create_root()?;

        Ok(fs)
    }

    /// Create a new SQL filesystem with an existing connection.
    pub fn with_connection(conn: Connection) -> Result<Self, FSError> {
        let conn = Arc::new(Mutex::new(conn));
        let fs = Self {
            config: SqlConfig::default(),
            conn,
        };

        fs.initialize_schema()?;
        fs.create_root()?;

        Ok(fs)
    }

    /// Initialize the database schema.
    fn initialize_schema(&self) -> Result<(), FSError> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                data BLOB,
                is_dir BOOLEAN NOT NULL,
                mode INTEGER NOT NULL,
                created_at INTEGER,
                modified_at INTEGER,
                accessed_at INTEGER
            )
            "#,
            [],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to create schema: {}", e),
        })?;

        conn.execute(
            r#"
            CREATE INDEX IF NOT EXISTS idx_files_is_dir ON files(is_dir)
            "#,
            [],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to create index: {}", e),
        })?;

        Ok(())
    }

    /// Create the root directory.
    fn create_root(&self) -> Result<(), FSError> {
        let path = "/";
        let now = Utc::now().timestamp_millis();
        let mode = S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;

        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT OR IGNORE INTO files (path, data, is_dir, mode, created_at, modified_at, accessed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![&path, &Vec::<u8>::new(), &true, &mode, &now, &now, &now],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to create root: {}", e),
        })?;

        Ok(())
    }

    /// Convert a Path to a normalized string for storage.
    pub(super) fn path_to_str(path: &Path) -> Result<String, FSError> {
        let path_str = path.to_str().ok_or_else(|| FSError::InvalidInput {
            message: "Path contains invalid UTF-8".to_string(),
        })?;

        Ok(if path_str.starts_with('/') {
            path_str.to_string()
        } else {
            format!("/{}", path_str)
        })
    }

    /// Get metadata for a path.
    pub(super) fn get_metadata(&self, path_str: &str) -> Result<FileMetadata, FSError> {
        let conn = self.conn.lock();

        let mut stmt = conn
            .prepare(
                r#"
                SELECT path, data, is_dir, mode, created_at, modified_at, accessed_at
                FROM files
                WHERE path = ?1
                "#,
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let row = stmt
            .query_row(params![path_str], |row| {
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
            .optional()
            .map_err(|e| FSError::Io {
                message: format!("Failed to query metadata: {}", e),
            })?;

        let (path, data, _is_dir, mode, created, modified, accessed) =
            row.ok_or_else(|| FSError::NotFound {
                path: PathBuf::from(path_str),
            })?;

        let size = data.unwrap_or_default().len() as u64;

        Ok(FileMetadata {
            path: PathBuf::from(path),
            size,
            mode,
            created: created.and_then(DateTime::from_timestamp_millis),
            modified: modified.and_then(DateTime::from_timestamp_millis),
            accessed: accessed.and_then(DateTime::from_timestamp_millis),
            is_symlink: false,
            symlink_target: None,
        })
    }

    /// Check if a path exists.
    pub(super) fn exists(&self, path_str: &str) -> Result<bool, FSError> {
        let conn = self.conn.lock();

        let mut stmt = conn
            .prepare("SELECT 1 FROM files WHERE path = ?1 LIMIT 1")
            .map_err(|e| FSError::Io {
                message: format!("Failed to prepare exists query: {}", e),
            })?;

        let exists = stmt
            .query_row(params![path_str], |_| Ok(true))
            .optional()
            .map_err(|e| FSError::Io {
                message: format!("Failed to query exists: {}", e),
            })?
            .unwrap_or(false);

        Ok(exists)
    }

    /// Check if the parent of a path exists and is a directory.
    pub(super) fn ensure_parent_exists(&self, path_str: &str) -> Result<(), FSError> {
        let path = PathBuf::from(path_str);

        if let Some(parent) = path.parent() {
            let parent_str = Self::path_to_str(parent)?;
            if parent_str != "/" {
                let metadata = self.get_metadata(&parent_str)?;
                if !metadata.is_directory() {
                    return Err(FSError::NotADirectory {
                        path: PathBuf::from(parent_str),
                    });
                }
            }
        }
        Ok(())
    }

    /// Check if a path is a directory.
    pub(super) fn is_dir(&self, path_str: &str) -> Result<bool, FSError> {
        let conn = self.conn.lock();

        let mut stmt = conn
            .prepare("SELECT is_dir FROM files WHERE path = ?1")
            .map_err(|e| FSError::Io {
                message: format!("Failed to prepare is_dir query: {}", e),
            })?;

        let is_dir = stmt
            .query_row(params![path_str], |row| row.get::<_, bool>(0))
            .optional()
            .map_err(|e| FSError::Io {
                message: format!("Failed to query is_dir: {}", e),
            })?;

        is_dir.ok_or_else(|| FSError::NotFound {
            path: PathBuf::from(path_str),
        })
    }
}

#[async_trait]
impl FileSystem for SqlFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let path_str = Self::path_to_str(path)?;

        if self.is_dir(&path_str)? {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let conn = self.conn.lock();

        let mut stmt = conn
            .prepare("SELECT data FROM files WHERE path = ?1")
            .map_err(|e| FSError::Io {
                message: format!("Failed to prepare truncate query: {}", e),
            })?;

        let existing_data: Vec<u8> = stmt
            .query_row(params![path_str], |row| {
                row.get::<_, Option<Vec<u8>>>(0)
                    .map(|opt| opt.unwrap_or_default())
            })
            .map_err(|e| FSError::Io {
                message: format!("Failed to read for truncate: {}", e),
            })?;

        let new_data = {
            let current_size = existing_data.len() as u64;
            if size < current_size {
                existing_data[..size as usize].to_vec()
            } else {
                let mut result = existing_data;
                result.resize(size as usize, 0);
                result
            }
        };

        let now = Utc::now().timestamp_millis();
        conn.execute(
            r#"
            UPDATE files
            SET data = ?1, modified_at = ?2
            WHERE path = ?3
            "#,
            params![&new_data, &now, &path_str],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to truncate: {}", e),
        })?;

        tracing::debug!(path = %path.display(), size = size, "truncated");
        Ok(())
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let path_str = Self::path_to_str(path)?;

        let now = Utc::now().timestamp_millis();

        if self.exists(&path_str)? {
            let conn = self.conn.lock();
            conn.execute(
                r#"
                UPDATE files
                SET accessed_at = ?1, modified_at = ?2
                WHERE path = ?3
                "#,
                params![&now, &now, &path_str],
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to touch: {}", e),
            })?;
        } else {
            self.create(path).await?;
            let conn = self.conn.lock();
            conn.execute(
                r#"
                UPDATE files
                SET accessed_at = ?1, modified_at = ?2
                WHERE path = ?3
                "#,
                params![&now, &now, &path_str],
            )
            .map_err(|e| FSError::Io {
                message: format!("Failed to touch: {}", e),
            })?;
        }

        tracing::debug!(path = %path.display(), "touched");
        Ok(())
    }
}
