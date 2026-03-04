use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::MemFS;

#[async_trait]
impl ChmodFS for MemFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let mut files = self.files.write();

        let entry = files.get_mut(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        entry.metadata.set_permissions(mode);
        entry.metadata.modified = Some(Utc::now());

        tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
        self.inc_op("chmod");
        Ok(())
    }
}
