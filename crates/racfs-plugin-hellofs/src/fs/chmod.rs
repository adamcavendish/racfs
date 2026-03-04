use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::HelloFS;

#[async_trait]
impl ChmodFS for HelloFS {
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}
