use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::{HttpFS, parse_request_id};

#[async_trait]
impl WriteFS for HttpFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        self.ensure_parent_exists(path)?;

        let mut files = self.files.write();

        if files.contains_key(path) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        if let Some(parent) = path.parent() {
            let parent_str = parent.to_string_lossy();
            if parent_str == "/requests" || parent_str == "/responses" || parent_str == "/cache" {
                return Err(FSError::PermissionDenied {
                    path: path.to_path_buf(),
                });
            }
        }

        let metadata = racfs_core::metadata::FileMetadata::file(path.to_path_buf(), 0);
        files.insert(
            path.to_path_buf(),
            super::FileEntry {
                data: Vec::new(),
                metadata,
            },
        );

        tracing::debug!(path = %path.display(), "created file");
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let mut files = self.files.write();

        let entry = files.get_mut(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        if path.ends_with("trigger") {
            let trigger_value = String::from_utf8_lossy(data).to_string();
            if trigger_value.trim() == "send" {
                drop(files);
                if let Ok(request_id) = parse_request_id(path) {
                    self.execute_request_sync(&request_id)?;
                }
                return Ok(data.len() as u64);
            }
        }

        if path.starts_with("/responses/") {
            return Err(FSError::ReadOnly);
        }

        let new_size = if flags.contains_append() || offset as usize >= entry.data.len() {
            let end = offset.max(0) as usize;
            if end > entry.data.len() {
                entry.data.resize(end, 0);
            }
            entry.data.extend_from_slice(data);
            entry.data.len()
        } else {
            entry.data = data.to_vec();
            data.len()
        };

        entry.metadata.size = new_size as u64;
        entry.metadata.modified = Some(chrono::Utc::now());

        tracing::debug!(path = %path.display(), bytes = data.len(), "wrote");
        Ok(data.len() as u64)
    }
}
