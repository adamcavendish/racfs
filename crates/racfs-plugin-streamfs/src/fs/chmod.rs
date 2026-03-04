use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::StreamFS;

#[async_trait]
impl ChmodFS for StreamFS {
    async fn chmod(&self, path: &Path, _mode: u32) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        Err(FSError::PermissionDenied { path: normalized })
    }
}
