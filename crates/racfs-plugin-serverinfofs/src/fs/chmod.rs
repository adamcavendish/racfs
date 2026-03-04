use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::ServerInfoFS;

#[async_trait]
impl ChmodFS for ServerInfoFS {
    async fn chmod(&self, path: &Path, _mode: u32) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }
}
