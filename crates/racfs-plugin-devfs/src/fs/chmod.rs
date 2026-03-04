use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::DevFS;

#[async_trait]
impl ChmodFS for DevFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        // Check if device exists
        self.get_device(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        let _ = mode;
        Ok(())
    }
}
