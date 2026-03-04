use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::DevFS;

#[async_trait]
impl ReadFS for DevFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let device_type = self.get_device(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        // offset is ignored for devices
        let _ = offset;

        let size = if size < 0 { 4096 } else { size as usize };

        match device_type {
            super::DeviceType::Null => {
                // /dev/null always returns EOF
                Ok(Vec::new())
            }
            super::DeviceType::Zero => {
                // /dev/zero returns zeros
                Ok(vec![0u8; size])
            }
            super::DeviceType::Random | super::DeviceType::URandom => {
                // /dev/random and /dev/urandom return random bytes
                let mut buf = vec![0u8; size];
                let mut seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                for byte in &mut buf {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    *byte = (seed >> 16) as u8;
                }
                Ok(buf)
            }
        }
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let device_type = self.get_device(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        Ok(Self::create_device_metadata(path, device_type))
    }
}
