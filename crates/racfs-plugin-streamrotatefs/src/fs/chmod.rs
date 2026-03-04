use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::StreamRotateFS;

#[async_trait]
impl ChmodFS for StreamRotateFS {
    async fn chmod(&self, path: &Path, _mode: u32) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;
        Ok(())
    }
}
