use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::HelloFS;

#[async_trait]
impl WriteFS for HelloFS {
    async fn create(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn write(
        &self,
        _path: &Path,
        _data: &[u8],
        _offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        Err(FSError::ReadOnly)
    }
}
