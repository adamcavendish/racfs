use std::path::Path;

use async_trait::async_trait;
use racfs_core::{
    error::FSError,
    filesystem::WriteFS,
    flags::WriteFlags,
    metadata::{FileMetadata, S_IFREG},
};
use rusqlite::params;

use super::KvFS;

#[async_trait]
impl WriteFS for KvFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        match &self.backend {
            super::KvBackend::Memory(store, metadata) => {
                let mut store = store.write();
                let mut metadata = metadata.write();

                if store.contains_key(path) {
                    return Err(FSError::AlreadyExists {
                        path: path.to_path_buf(),
                    });
                }

                store.insert(path.to_path_buf(), Vec::new());
                metadata.insert(
                    path.to_path_buf(),
                    FileMetadata::file(path.to_path_buf(), 0),
                );

                tracing::debug!(path = %path.display(), "created");
                Ok(())
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let mode = S_IFREG | 0o644;
                conn.execute(
                    "INSERT INTO kvfs (path, data, is_dir, mode) VALUES (?1, ?2, 0, ?3)",
                    params![&path_str, &Vec::<u8>::new(), mode],
                )
                .map_err(|e| FSError::Io {
                    message: format!("Failed to create: {}", e),
                })?;
                tracing::debug!(path = %path.display(), "created");
                Ok(())
            }
        }
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        match &self.backend {
            super::KvBackend::Memory(store, metadata) => {
                let mut store = store.write();
                let mut metadata = metadata.write();

                if !store.contains_key(path) {
                    return Err(FSError::NotFound {
                        path: path.to_path_buf(),
                    });
                }

                let entry = store.get_mut(path).expect("key exists");
                let data_len = data.len() as u64;

                if flags.contains_append() {
                    entry.extend_from_slice(data);
                } else if offset as usize >= entry.len() {
                    entry.resize(offset as usize, 0);
                    entry.extend_from_slice(data);
                } else {
                    entry.splice(
                        offset as usize..offset as usize + data.len(),
                        data.iter().copied(),
                    );
                }

                if let Some(meta) = metadata.get_mut(path) {
                    meta.size = entry.len() as u64;
                }

                tracing::debug!(path = %path.display(), bytes = data.len(), "wrote");
                Ok(data_len)
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let mut existing: Vec<u8> = conn
                    .query_row(
                        "SELECT data FROM kvfs WHERE path = ?1 AND is_dir = 0",
                        params![&path_str],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .map_err(|_| FSError::NotFound {
                        path: path.to_path_buf(),
                    })?;

                let data_len = data.len() as u64;

                if flags.contains_append() {
                    existing.extend_from_slice(data);
                } else if offset as usize >= existing.len() {
                    existing.resize(offset as usize, 0);
                    existing.extend_from_slice(data);
                } else {
                    existing.splice(
                        offset as usize..offset as usize + data.len(),
                        data.iter().copied(),
                    );
                }

                conn.execute(
                    "UPDATE kvfs SET data = ?1 WHERE path = ?2",
                    params![&existing, &path_str],
                )
                .map_err(|e| FSError::Io {
                    message: format!("Failed to write: {}", e),
                })?;

                tracing::debug!(path = %path.display(), bytes = data.len(), "wrote");
                Ok(data_len)
            }
        }
    }
}
