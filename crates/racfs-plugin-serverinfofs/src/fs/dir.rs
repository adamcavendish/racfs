use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::ServerInfoFS;

#[async_trait]
impl DirFS for ServerInfoFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let entries = self.entries();
        let path_str = path.to_string_lossy();
        let prefix = if path_str == "/" {
            String::new()
        } else {
            path_str.to_string()
        };

        let result: Vec<FileMetadata> = entries
            .iter()
            .filter(|e| {
                let e_str = e.path.to_string_lossy();
                e_str.starts_with(&prefix) && e_str != prefix
            })
            .filter_map(|e| {
                let relative = if prefix.is_empty() {
                    e.path.clone()
                } else {
                    e.path
                        .strip_prefix(&prefix)
                        .unwrap_or(&e.path)
                        .to_path_buf()
                };

                if relative.as_os_str() == "/" {
                    return None;
                }

                let components: Vec<_> = relative.components().collect();
                if components.len() == 1
                    || (components.len() == 2 && components[0] == std::path::Component::RootDir)
                {
                    let mut metadata = e.metadata.clone();
                    metadata.path = e.path.clone();
                    Some(metadata)
                } else {
                    None
                }
            })
            .collect();

        Ok(result)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }

    async fn rename(&self, old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: old_path.to_path_buf(),
        })
    }
}
