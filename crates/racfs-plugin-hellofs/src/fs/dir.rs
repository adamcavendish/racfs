use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::HelloFS;

#[async_trait]
impl DirFS for HelloFS {
    async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let files = self.files.read();

        let path_str = path.to_string_lossy();
        let prefix = if path_str == "/" {
            "".to_string()
        } else {
            path_str.to_string()
        };

        let entries: Vec<FileMetadata> = files
            .iter()
            .filter(|(p, _)| {
                let p_str = p.to_string_lossy();
                p_str.starts_with(&prefix) && p_str != prefix
            })
            .filter_map(|(p, entry)| {
                let relative = if prefix.is_empty() {
                    p.clone()
                } else {
                    p.strip_prefix(&prefix).unwrap_or(p).to_path_buf()
                };

                let components: Vec<_> = relative.components().collect();

                if relative.as_os_str() == "/" {
                    return None;
                }

                if components.len() == 1
                    || (components.len() == 2 && components[0] == std::path::Component::RootDir)
                {
                    let mut metadata = entry.metadata.clone();
                    let path_str = relative.to_string_lossy();
                    metadata.path = if path_str.starts_with('/') {
                        relative
                    } else {
                        PathBuf::from("/").join(&relative)
                    };
                    Some(metadata)
                } else {
                    None
                }
            })
            .collect();

        Ok(entries)
    }

    async fn remove(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn remove_all(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}
