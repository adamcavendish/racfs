use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{
    error::FSError,
    filesystem::DirFS,
    metadata::{FileMetadata, S_IFDIR},
};
use rusqlite::params;

use super::KvFS;

#[async_trait]
impl DirFS for KvFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        match &self.backend {
            super::KvBackend::Memory(_store, metadata) => {
                let mut metadata = metadata.write();

                if metadata.contains_key(path) {
                    return Err(FSError::AlreadyExists {
                        path: path.to_path_buf(),
                    });
                }

                metadata.insert(
                    path.to_path_buf(),
                    FileMetadata::directory(path.to_path_buf()),
                );

                tracing::debug!(path = %path.display(), "created directory");
                Ok(())
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let mode = S_IFDIR | 0o755;
                conn.execute(
                    "INSERT INTO kvfs (path, data, is_dir, mode) VALUES (?1, ?2, 1, ?3)",
                    params![&path_str, &Vec::<u8>::new(), mode],
                )
                .map_err(|e| FSError::Io {
                    message: format!("Failed to mkdir: {}", e),
                })?;
                tracing::debug!(path = %path.display(), "created directory");
                Ok(())
            }
        }
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        match &self.backend {
            super::KvBackend::Memory(_store, metadata) => {
                let metadata = metadata.read();

                let prefix = path.to_string_lossy();
                let prefix_str = prefix.to_string();
                let entries: Vec<FileMetadata> = metadata
                    .iter()
                    .filter(|(p, _)| {
                        let p_str = p.to_string_lossy();
                        p_str.starts_with(&prefix_str) && p_str != prefix_str
                    })
                    .filter_map(|(p, meta)| {
                        let relative = if prefix.is_empty() {
                            p.clone()
                        } else {
                            p.strip_prefix(&prefix_str).unwrap_or(p).to_path_buf()
                        };

                        let components: Vec<_> = relative.components().collect();
                        if components.len() == 1 {
                            Some(meta.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                Ok(entries)
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let prefix_check = if path_str == "/" {
                    "/"
                } else {
                    path_str.trim_end_matches('/')
                };
                let prefix_like = if path_str == "/" {
                    "/%".to_string()
                } else {
                    format!("{}/%", prefix_check)
                };
                let prefix_with_slash = if path_str == "/" {
                    "/".to_string()
                } else {
                    format!("{}/", prefix_check)
                };

                let conn = conn.lock();
                let mut stmt = conn
                    .prepare(
                        "SELECT path, data, is_dir, mode FROM kvfs WHERE path LIKE ?1 AND path != ?2",
                    )
                    .map_err(|e| FSError::Io {
                        message: format!("Failed to prepare read_dir: {}", e),
                    })?;

                let rows = stmt
                    .query_map(params![&prefix_like, &path_str], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, i32>(2)?,
                            row.get::<_, i64>(3)? as u32,
                        ))
                    })
                    .map_err(|e| FSError::Io {
                        message: format!("Failed to read_dir: {}", e),
                    })?;

                let mut entries = Vec::new();
                for row in rows {
                    let (path_buf, data, is_dir, mode) = row.map_err(|e| FSError::Io {
                        message: format!("Row error: {}", e),
                    })?;
                    let rest = path_buf
                        .strip_prefix(&prefix_with_slash)
                        .unwrap_or(path_buf.as_str());
                    if rest.contains('/') {
                        continue;
                    }
                    let size = if is_dir != 0 { 0 } else { data.len() as u64 };
                    entries.push(FileMetadata {
                        path: PathBuf::from(path_buf),
                        size,
                        mode,
                        created: None,
                        modified: None,
                        accessed: None,
                        is_symlink: false,
                        symlink_target: None,
                    });
                }
                Ok(entries)
            }
        }
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        match &self.backend {
            super::KvBackend::Memory(store, metadata) => {
                let mut store = store.write();
                let mut metadata = metadata.write();

                if store.remove(path).is_none() && metadata.remove(path).is_none() {
                    return Err(FSError::NotFound {
                        path: path.to_path_buf(),
                    });
                }

                tracing::debug!(path = %path.display(), "removed");
                Ok(())
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let changed = conn
                    .execute("DELETE FROM kvfs WHERE path = ?1", params![&path_str])
                    .map_err(|e| FSError::Io {
                        message: format!("Failed to remove: {}", e),
                    })?;
                if changed == 0 {
                    return Err(FSError::NotFound {
                        path: path.to_path_buf(),
                    });
                }
                tracing::debug!(path = %path.display(), "removed");
                Ok(())
            }
        }
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        match &self.backend {
            super::KvBackend::Memory(store, metadata) => {
                let mut store = store.write();
                let mut metadata = metadata.write();

                let keys: Vec<PathBuf> = store
                    .keys()
                    .filter(|p| p.starts_with(path) || p.as_path() == path)
                    .cloned()
                    .collect();

                for key in keys {
                    store.remove(&key);
                    metadata.remove(&key);
                }

                tracing::debug!(path = %path.display(), "removed all");
                Ok(())
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let prefix = path_str.trim_end_matches('/');
                let prefix_like = format!("{}/%", prefix);
                let conn = conn.lock();
                conn.execute(
                    "DELETE FROM kvfs WHERE path = ?1 OR path LIKE ?2",
                    params![&path_str, &prefix_like],
                )
                .map_err(|e| FSError::Io {
                    message: format!("Failed to remove_all: {}", e),
                })?;
                tracing::debug!(path = %path.display(), "removed all");
                Ok(())
            }
        }
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        match &self.backend {
            super::KvBackend::Memory(store, metadata) => {
                let mut store = store.write();
                let mut metadata = metadata.write();

                let data = store.remove(old_path).ok_or_else(|| FSError::NotFound {
                    path: old_path.to_path_buf(),
                })?;

                let meta = metadata.remove(old_path).ok_or_else(|| FSError::NotFound {
                    path: old_path.to_path_buf(),
                })?;

                if store.contains_key(new_path) {
                    store.insert(old_path.to_path_buf(), data);
                    metadata.insert(old_path.to_path_buf(), meta);
                    return Err(FSError::AlreadyExists {
                        path: new_path.to_path_buf(),
                    });
                }

                store.insert(new_path.to_path_buf(), data);
                metadata.insert(new_path.to_path_buf(), meta);

                tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed");
                Ok(())
            }
            super::KvBackend::Sqlite(conn) => {
                let old_str = KvFS::path_to_str(old_path)?;
                let new_str = KvFS::path_to_str(new_path)?;
                let conn = conn.lock();
                let exists: i32 = conn
                    .query_row(
                        "SELECT COUNT(1) FROM kvfs WHERE path = ?1",
                        params![&new_str],
                        |row| row.get(0),
                    )
                    .map_err(|e| FSError::Io {
                        message: format!("Failed to check rename target: {}", e),
                    })?;
                if exists != 0 {
                    return Err(FSError::AlreadyExists {
                        path: new_path.to_path_buf(),
                    });
                }
                conn.execute(
                    "UPDATE kvfs SET path = ?1 WHERE path = ?2",
                    params![&new_str, &old_str],
                )
                .map_err(|e| FSError::Io {
                    message: format!("Failed to rename: {}", e),
                })?;
                let changed = conn.changes();
                if changed == 0 {
                    return Err(FSError::NotFound {
                        path: old_path.to_path_buf(),
                    });
                }
                tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed");
                Ok(())
            }
        }
    }
}
