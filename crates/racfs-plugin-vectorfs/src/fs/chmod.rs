use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::VectorFS;

#[async_trait]
impl ChmodFS for VectorFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        if self.is_virtual_path(path) && path.to_string_lossy().starts_with("/index/") {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        let path_str = path.to_string_lossy().to_string();
        let has_meta = self.metadata.read().contains_key(path);
        if !has_meta {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        {
            let mut metadata = self.metadata.write();
            if let Some(meta) = metadata.get_mut(path) {
                meta.set_permissions(mode);
                meta.modified = Some(Utc::now());
            }
        }

        if path_str.starts_with("/documents/")
            && let Some((data, vector, _, created_at, _)) = self.get_document(path).await?
        {
            let modified_at = Some(Utc::now().timestamp_millis());
            self.persist_document(&path_str, &data, &vector, mode, created_at, modified_at)
                .await?;
        }

        if let Some(meta) = self.metadata.write().get_mut(path) {
            meta.set_permissions(mode);
            meta.modified = Some(Utc::now());
        }

        tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
        Ok(())
    }
}
