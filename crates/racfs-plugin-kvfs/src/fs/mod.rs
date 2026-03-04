//! Key-Value store filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use racfs_core::{
    error::FSError,
    filesystem::FileSystem,
    metadata::{FileMetadata, S_IFDIR},
};
use rusqlite::Connection;

/// Backing store for KvFS: in-memory HashMaps or SQLite.
pub enum KvBackend {
    Memory(
        Arc<RwLock<HashMap<PathBuf, Vec<u8>>>>,
        Arc<RwLock<HashMap<PathBuf, FileMetadata>>>,
    ),
    Sqlite(Arc<Mutex<Connection>>),
}

/// Key-value store filesystem (in-memory or SQLite-backed).
pub struct KvFS {
    pub(crate) backend: KvBackend,
}

impl KvFS {
    /// Create a new in-memory KV filesystem.
    pub fn new() -> Self {
        Self {
            backend: KvBackend::Memory(
                Arc::new(RwLock::new(HashMap::new())),
                Arc::new(RwLock::new(HashMap::new())),
            ),
        }
    }

    /// Create a KV filesystem with initial data (in-memory).
    pub fn with_data(data: HashMap<PathBuf, Vec<u8>>) -> Self {
        let metadata: HashMap<PathBuf, FileMetadata> = data
            .keys()
            .map(|path| {
                let meta = FileMetadata::file(
                    path.clone(),
                    data.get(path).map(|d| d.len() as u64).unwrap_or(0),
                );
                (path.clone(), meta)
            })
            .collect();

        Self {
            backend: KvBackend::Memory(
                Arc::new(RwLock::new(data)),
                Arc::new(RwLock::new(metadata)),
            ),
        }
    }

    /// Create a SQLite-backed KV filesystem; data persists across restarts.
    pub fn with_database(path: impl AsRef<Path>) -> Result<Self, FSError> {
        let path = path.as_ref();
        let conn = Connection::open(path).map_err(|e| FSError::Io {
            message: format!("Failed to open database: {}", e),
        })?;

        let conn = Arc::new(Mutex::new(conn));
        let fs = Self {
            backend: KvBackend::Sqlite(conn),
        };

        fs.initialize_schema()?;
        fs.create_root()?;

        Ok(fs)
    }

    /// Normalize path to a string with leading slash for storage.
    pub(crate) fn path_to_str(path: &Path) -> Result<String, FSError> {
        let path_str = path.to_str().ok_or_else(|| FSError::InvalidInput {
            message: "Path contains invalid UTF-8".to_string(),
        })?;

        Ok(if path_str.starts_with('/') {
            path_str.to_string()
        } else {
            format!("/{}", path_str)
        })
    }

    fn initialize_schema(&self) -> Result<(), FSError> {
        let KvBackend::Sqlite(conn) = &self.backend else {
            return Ok(());
        };
        let conn = conn.lock();
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS kvfs (
                path TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                is_dir INTEGER NOT NULL,
                mode INTEGER NOT NULL
            )
            "#,
            [],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to create schema: {}", e),
        })?;
        Ok(())
    }

    fn create_root(&self) -> Result<(), FSError> {
        let KvBackend::Sqlite(conn) = &self.backend else {
            return Ok(());
        };
        let path = "/";
        let mode = S_IFDIR | 0o755;
        let conn = conn.lock();
        conn.execute(
            r#"
            INSERT OR IGNORE INTO kvfs (path, data, is_dir, mode)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            rusqlite::params![path, &Vec::<u8>::new(), 1i32, mode],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to create root: {}", e),
        })?;
        Ok(())
    }
}

impl Default for KvFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for KvFS {}
