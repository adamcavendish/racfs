use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::LocalFS;

#[async_trait]
impl DirFS for LocalFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&resolved).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            let entries: Vec<FileMetadata> = std::fs::read_dir(&resolved)
                .map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let metadata = entry.metadata().ok()?;
                    let path = entry.path();

                    let mode = if metadata.is_dir() {
                        0o040000 | 0o755
                    } else if metadata.is_symlink() {
                        0o120000 | 0o755
                    } else {
                        0o100000 | 0o644
                    };

                    Some(FileMetadata {
                        path,
                        size: metadata.len(),
                        mode,
                        created: None,
                        modified: metadata.modified().ok().map(|t| t.into()),
                        accessed: metadata.accessed().ok().map(|t| t.into()),
                        is_symlink: metadata.is_symlink(),
                        symlink_target: None,
                    })
                })
                .collect();
            Ok(entries)
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;
        let path_buf = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let metadata = std::fs::metadata(&resolved).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    FSError::NotFound { path: path_buf }
                } else {
                    FSError::Io {
                        message: e.to_string(),
                    }
                }
            })?;

            if metadata.is_dir() {
                std::fs::remove_dir(&resolved).map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?;
            } else {
                std::fs::remove_file(&resolved).map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?;
            }
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;
        let path_buf = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if !resolved.exists() {
                return Err(FSError::NotFound { path: path_buf });
            }
            std::fs::remove_dir_all(&resolved).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let resolved_old = self.resolve(old_path)?;
        let resolved_new = self.resolve(new_path)?;

        tokio::task::spawn_blocking(move || {
            std::fs::rename(&resolved_old, &resolved_new).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }
}
