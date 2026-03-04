use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::HttpFS;

#[async_trait]
impl ChmodFS for HttpFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let mut files = self.files.write();

        let entry = files.get_mut(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        entry.metadata.set_permissions(mode);
        entry.metadata.modified = Some(chrono::Utc::now());

        tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
        Ok(())
    }
}
