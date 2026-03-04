use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::DevFS;

#[async_trait]
impl WriteFS for DevFS {
    async fn create(&self, _path: &Path) -> Result<(), FSError> {
        // Device files must be created via register_device
        Err(FSError::NotSupported {
            message: "cannot create device files manually".to_string(),
        })
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        _offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let device_type = self.get_device(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        match device_type {
            super::DeviceType::Null => {
                // /dev/null discards all data
                Ok(data.len() as u64)
            }
            super::DeviceType::Zero | super::DeviceType::Random | super::DeviceType::URandom => {
                Ok(data.len() as u64)
            }
        }
    }
}
