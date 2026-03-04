use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::S3FS;

#[async_trait]
impl ChmodFS for S3FS {
    async fn chmod(&self, path: &Path, _mode: u32) -> Result<(), FSError> {
        // S3 doesn't support POSIX permissions
        // Return Ok(()) as a no-op since S3 ACLs are different
        tracing::debug!(path = %path.display(), "chmod (no-op for S3)");
        Ok(())
    }
}
