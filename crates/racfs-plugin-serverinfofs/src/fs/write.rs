use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::ServerInfoFS;

#[async_trait]
impl WriteFS for ServerInfoFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }

    async fn write(
        &self,
        path: &Path,
        _data: &[u8],
        _offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }
}
