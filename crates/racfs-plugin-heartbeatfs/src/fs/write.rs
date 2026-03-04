use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::HeartbeatFS;

#[async_trait]
impl WriteFS for HeartbeatFS {
    async fn create(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        if path != Path::new("/pulse") {
            return Err(FSError::ReadOnly);
        }

        if offset != 0 {
            return Err(FSError::InvalidInput {
                message: "offset must be 0".to_string(),
            });
        }

        let content = std::str::from_utf8(data).map_err(|_| FSError::InvalidInput {
            message: "invalid UTF-8".to_string(),
        })?;

        if content.trim() != "beat" {
            return Err(FSError::InvalidInput {
                message: "must write 'beat' to /pulse".to_string(),
            });
        }

        self.inner.write().heartbeat();

        tracing::debug!("heartbeat recorded");
        Ok(data.len() as u64)
    }
}
