use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::QueueFS;

#[async_trait]
impl ChmodFS for QueueFS {
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "chmod not supported in queuefs".to_string(),
        })
    }
}
