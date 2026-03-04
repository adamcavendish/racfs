use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::DevFS;

#[async_trait]
impl DirFS for DevFS {
    async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "cannot create directories in devfs".to_string(),
        })
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        if path != "/" && path != "" {
            return Err(FSError::NotADirectory {
                path: path.to_path_buf(),
            });
        }

        let devices = self.devices.read();
        let entries: Vec<FileMetadata> = devices
            .iter()
            .map(|(path, device_type)| Self::create_device_metadata(path, *device_type))
            .collect();

        Ok(entries)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let mut devices = self.devices.write();
        devices
            .remove(path)
            .map(|_| ())
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        self.remove(path).await
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "cannot rename device files".to_string(),
        })
    }
}
