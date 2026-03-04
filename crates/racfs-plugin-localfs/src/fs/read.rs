use std::io::{Read, Seek};
use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::LocalFS;

#[async_trait]
impl ReadFS for LocalFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            let mut file = std::fs::File::open(&resolved).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;

            if offset > 0 {
                file.seek(std::io::SeekFrom::Start(offset as u64))
                    .map_err(|e| FSError::Io {
                        message: e.to_string(),
                    })?;
            }

            let size = if size < 0 { 64 * 1024 } else { size as usize };

            let mut buffer = vec![0u8; size];
            let bytes_read = file.read(&mut buffer).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;

            buffer.truncate(bytes_read);
            Ok(buffer)
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let resolved = self.resolve(path)?;
        let path_buf = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let metadata = std::fs::metadata(&resolved).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    FSError::NotFound {
                        path: path_buf.clone(),
                    }
                } else {
                    FSError::Io {
                        message: e.to_string(),
                    }
                }
            })?;

            let mode = if metadata.is_dir() {
                0o040000 | 0o755
            } else if metadata.is_symlink() {
                0o120000 | 0o755
            } else {
                0o100000 | 0o644
            };

            Ok(FileMetadata {
                path: path_buf,
                size: metadata.len(),
                mode,
                created: metadata.created().ok().map(|t| t.into()),
                modified: metadata.modified().ok().map(|t| t.into()),
                accessed: metadata.accessed().ok().map(|t| t.into()),
                is_symlink: metadata.is_symlink(),
                symlink_target: None,
            })
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }
}
