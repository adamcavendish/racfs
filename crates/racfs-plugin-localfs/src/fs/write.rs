use std::io::{Seek, Write};
use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::LocalFS;

#[async_trait]
impl WriteFS for LocalFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            // Create parent directories if needed
            if let Some(parent) = resolved.parent() {
                std::fs::create_dir_all(parent).map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?;
            }

            std::fs::File::create(&resolved).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;

            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let resolved = self.resolve(path)?;
        let data = data.to_vec();

        tokio::task::spawn_blocking(move || {
            if let Some(parent) = resolved.parent() {
                std::fs::create_dir_all(parent).map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?;
            }

            let mut file = if flags.contains_append() || offset > 0 {
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(flags.contains_append())
                    .open(&resolved)
                    .map_err(|e| FSError::Io {
                        message: e.to_string(),
                    })?;

                if offset > 0 && !flags.contains_append() {
                    f.seek(std::io::SeekFrom::Start(offset as u64))
                        .map_err(|e| FSError::Io {
                            message: e.to_string(),
                        })?;
                }
                f
            } else {
                std::fs::File::create(&resolved).map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?
            };

            let written = file.write(&data).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;
            Ok(written as u64)
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }
}
