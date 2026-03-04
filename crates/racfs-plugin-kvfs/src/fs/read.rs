use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};
use rusqlite::params;

use super::KvFS;

#[async_trait]
impl ReadFS for KvFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        match &self.backend {
            super::KvBackend::Memory(store, _metadata) => {
                let store = store.read();
                let data = store.get(path).ok_or_else(|| FSError::NotFound {
                    path: path.to_path_buf(),
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
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let data: Vec<u8> = conn
                    .query_row(
                        "SELECT data FROM kvfs WHERE path = ?1 AND is_dir = 0",
                        params![&path_str],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .map_err(|_| FSError::NotFound {
                        path: path.to_path_buf(),
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
        }
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        match &self.backend {
            super::KvBackend::Memory(_store, metadata) => {
                let metadata = metadata.read();
                metadata
                    .get(path)
                    .cloned()
                    .ok_or_else(|| FSError::NotFound {
                        path: path.to_path_buf(),
                    })
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let (path_buf, data, is_dir, mode): (String, Vec<u8>, i32, u32) = conn
                    .query_row(
                        "SELECT path, data, is_dir, mode FROM kvfs WHERE path = ?1",
                        params![&path_str],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Vec<u8>>(1)?,
                                row.get::<_, i32>(2)?,
                                row.get::<_, i64>(3)? as u32,
                            ))
                        },
                    )
                    .map_err(|_| FSError::NotFound {
                        path: path.to_path_buf(),
                    })?;

                let size = if is_dir != 0 { 0 } else { data.len() as u64 };
                let meta = FileMetadata {
                    path: PathBuf::from(path_buf),
                    size,
                    mode,
                    created: None,
                    modified: None,
                    accessed: None,
                    is_symlink: false,
                    symlink_target: None,
                };
                Ok(meta)
            }
        }
    }
}
