//! PostgreSQL-backed filesystem implementation.

#![allow(clippy::needless_borrows_for_generic_args)]

mod chmod;
mod dir;
mod read;
mod write;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use racfs_core::{
    error::FSError,
    filesystem::{FileSystem, WriteFS},
    metadata::{FileMetadata, S_IFDIR, S_IRGRP, S_IROTH, S_IRWXU, S_IXGRP, S_IXOTH},
};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

/// Configuration for the PostgreSQL filesystem.
#[derive(Debug, Clone)]
pub struct PgsConfig {
    /// PostgreSQL connection string (postgres://user:pass@host/db).
    pub database_url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
    /// Maximum file size in bytes.
    pub max_file_size: usize,
    /// Minimum idle connections.
    pub min_idle_connections: Option<u32>,
}

impl Default for PgsConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:postgres@localhost/racfs".to_string(),
            max_connections: 10,
            max_file_size: 5 * 1024 * 1024, // 5MB
            min_idle_connections: Some(2),
        }
    }
}

/// PostgreSQL-backed filesystem.
pub struct PgsFS {
    pub(super) config: PgsConfig,
    pub(super) pool: Arc<PgPool>,
}

impl PgsFS {
    /// Create a new PostgreSQL filesystem with default configuration.
    pub fn new() -> Result<Self, FSError> {
        Self::with_config(PgsConfig::default())
    }

    /// Create a new PostgreSQL filesystem with the given configuration.
    pub fn with_config(config: PgsConfig) -> Result<Self, FSError> {
        let pool_options = PgPoolOptions::new().max_connections(config.max_connections);

        let pool_options = match config.min_idle_connections {
            Some(min) => pool_options.min_connections(min),
            None => pool_options,
        };

        let pool =
            futures::executor::block_on(async { pool_options.connect(&config.database_url).await })
                .map_err(|e| FSError::Io {
                    message: format!("Failed to connect to PostgreSQL: {}", e),
                })?;

        let pool = Arc::new(pool);
        let fs = Self { config, pool };

        futures::executor::block_on(async { fs.initialize_schema().await })?;

        futures::executor::block_on(async { fs.create_root().await })?;

        Ok(fs)
    }

    /// Create a new PostgreSQL filesystem with an existing pool.
    pub fn with_pool(pool: PgPool) -> Result<Self, FSError> {
        let pool = Arc::new(pool);
        let fs = Self {
            config: PgsConfig::default(),
            pool,
        };

        futures::executor::block_on(async { fs.initialize_schema().await })?;

        futures::executor::block_on(async { fs.create_root().await })?;

        Ok(fs)
    }

    /// Initialize the database schema.
    async fn initialize_schema(&self) -> Result<(), FSError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                data BYTEA,
                is_dir BOOLEAN NOT NULL,
                mode INTEGER NOT NULL,
                created_at BIGINT,
                modified_at BIGINT,
                accessed_at BIGINT
            )
            "#,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to create schema: {}", e),
        })?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_files_is_dir ON files(is_dir)
            "#,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to create index: {}", e),
        })?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(path text_pattern_ops)
            "#,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to create path index: {}", e),
        })?;

        Ok(())
    }

    /// Create the root directory.
    async fn create_root(&self) -> Result<(), FSError> {
        let path = "/";
        let now = Utc::now().timestamp_millis();
        let mode = S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH;

        sqlx::query(
            r#"
            INSERT INTO files (path, data, is_dir, mode, created_at, modified_at, accessed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (path) DO NOTHING
            "#,
        )
        .bind(&path)
        .bind(&Vec::<u8>::new())
        .bind(&true)
        .bind(&(mode as i32))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(self.pool.as_ref())
        .await
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
    pub(super) async fn get_metadata(&self, path_str: &str) -> Result<FileMetadata, FSError> {
        let row = sqlx::query(
            r#"
            SELECT path, data, is_dir, mode, created_at, modified_at, accessed_at
            FROM files
            WHERE path = $1
            "#,
        )
        .bind(path_str)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to query metadata: {}", e),
        })?;

        let row = row.ok_or_else(|| FSError::NotFound {
            path: PathBuf::from(path_str),
        })?;

        let path: String = row.get("path");
        let data: Option<Vec<u8>> = row.get("data");
        let _is_dir: bool = row.get("is_dir");
        let mode: i32 = row.get("mode");
        let created: Option<i64> = row.get("created_at");
        let modified: Option<i64> = row.get("modified_at");
        let accessed: Option<i64> = row.get("accessed_at");

        let size = data.unwrap_or_default().len() as u64;

        Ok(FileMetadata {
            path: PathBuf::from(path),
            size,
            mode: mode as u32,
            created: created.and_then(DateTime::from_timestamp_millis),
            modified: modified.and_then(DateTime::from_timestamp_millis),
            accessed: accessed.and_then(DateTime::from_timestamp_millis),
            is_symlink: false,
            symlink_target: None,
        })
    }

    /// Check if a path exists.
    pub(super) async fn exists(&self, path_str: &str) -> Result<bool, FSError> {
        let row = sqlx::query("SELECT 1 FROM files WHERE path = $1 LIMIT 1")
            .bind(path_str)
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to query exists: {}", e),
            })?;

        Ok(row.is_some())
    }

    /// Check if the parent of a path exists and is a directory.
    pub(super) async fn ensure_parent_exists(&self, path_str: &str) -> Result<(), FSError> {
        let path = PathBuf::from(path_str);

        if let Some(parent) = path.parent() {
            let parent_str = Self::path_to_str(parent)?;
            if parent_str != "/" {
                let metadata = self.get_metadata(&parent_str).await?;
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
    pub(super) async fn is_dir(&self, path_str: &str) -> Result<bool, FSError> {
        let row = sqlx::query("SELECT is_dir FROM files WHERE path = $1")
            .bind(path_str)
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to query is_dir: {}", e),
            })?;

        let row = row.ok_or_else(|| FSError::NotFound {
            path: PathBuf::from(path_str),
        })?;

        let is_dir: bool = row.get("is_dir");
        Ok(is_dir)
    }
}

#[async_trait]
impl FileSystem for PgsFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let path_str = Self::path_to_str(path)?;

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
                message: format!("Failed to read for truncate: {}", e),
            })?;

        let existing_data: Vec<u8> = row.get::<Option<Vec<u8>>, _>("data").unwrap_or_default();

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
        sqlx::query(
            r#"
            UPDATE files
            SET data = $1, modified_at = $2
            WHERE path = $3
            "#,
        )
        .bind(&new_data)
        .bind(&now)
        .bind(&path_str)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to truncate: {}", e),
        })?;

        tracing::debug!(path = %path.display(), size = size, "truncated");
        Ok(())
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let path_str = Self::path_to_str(path)?;
        let now = Utc::now().timestamp_millis();

        if self.exists(&path_str).await? {
            sqlx::query(
                r#"
                UPDATE files
                SET accessed_at = $1, modified_at = $2
                WHERE path = $3
                "#,
            )
            .bind(now)
            .bind(now)
            .bind(&path_str)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to touch: {}", e),
            })?;
        } else {
            self.create(path).await?;
            sqlx::query(
                r#"
                UPDATE files
                SET accessed_at = $1, modified_at = $2
                WHERE path = $3
                "#,
            )
            .bind(now)
            .bind(now)
            .bind(&path_str)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| FSError::Io {
                message: format!("Failed to touch: {}", e),
            })?;
        }

        tracing::debug!(path = %path.display(), "touched");
        Ok(())
    }
}
