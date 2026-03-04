use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::StreamRotateFS;

#[async_trait]
impl WriteFS for StreamRotateFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        match path_str.as_str() {
            "/current" | "/rotate" | "/config" => Ok(()),
            _ => Err(FSError::AlreadyExists { path: normalized }),
        }
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        _offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        match path_str.as_str() {
            "/current" => {
                let mut state = self.state.write();
                state.current.data.extend_from_slice(data);
                let bytes_written = data.len() as u64;
                state.current.metadata.size = state.current.data.len() as u64;
                state.current.metadata.modified = Some(Utc::now());

                tracing::debug!(
                    bytes = bytes_written,
                    total_size = state.current.data.len(),
                    "wrote to /current"
                );

                drop(state);

                self.check_rotate()?;

                Ok(bytes_written)
            }
            "/rotate" => {
                let content = String::from_utf8_lossy(data);
                if content.trim() == "rotate" {
                    self.rotate()?;
                    Ok(data.len() as u64)
                } else {
                    Ok(data.len() as u64)
                }
            }
            "/config" => Err(FSError::PermissionDenied { path: normalized }),
            _ => Err(FSError::IsDirectory { path: normalized }),
        }
    }
}
