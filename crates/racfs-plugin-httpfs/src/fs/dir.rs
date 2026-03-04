use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::{FileEntry, HttpFS};

#[async_trait]
impl DirFS for HttpFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        if let Some(parent) = path.parent() {
            let parent_str = parent.to_string_lossy();
            if parent_str != "/"
                && parent_str != "/requests"
                && parent_str != "/responses"
                && parent_str != "/cache"
            {
                return Err(FSError::PermissionDenied {
                    path: path.to_path_buf(),
                });
            }
        }

        let mut files = self.files.write();

        if files.contains_key(path) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let metadata = FileMetadata::directory(path.to_path_buf());
        files.insert(
            path.to_path_buf(),
            FileEntry {
                data: Vec::new(),
                metadata,
            },
        );

        tracing::debug!(path = %path.display(), "created directory");
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let files = self.files.read();

        let path_str = path.to_string_lossy();
        let prefix = if path_str == "/" {
            "".to_string()
        } else {
            path_str.to_string()
        };

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
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

                if relative.as_os_str() == "/" {
                    return None;
                }

                let first_component = relative
                    .components()
                    .find(|c| matches!(c, std::path::Component::Normal(_)));

                let component_str = first_component?.as_os_str().to_string_lossy();

                if seen.contains(component_str.as_ref()) {
                    return None;
                }
                seen.insert(component_str.as_ref().to_string());

                let mut metadata = entry.metadata.clone();
                let child_path = if prefix.is_empty() {
                    PathBuf::from("/").join(component_str.as_ref())
                } else {
                    PathBuf::from(&prefix).join(component_str.as_ref())
                };
                metadata.path = child_path;
                Some(metadata)
            })
            .collect();

        Ok(entries)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let mut files = self.files.write();

        let entry = files.remove(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        if entry.metadata.is_directory() {
            let has_children = files.keys().any(|p| p != path && p.starts_with(path));
            if has_children {
                return Err(FSError::DirectoryNotEmpty);
            }
        }

        tracing::debug!(path = %path.display(), "removed");
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let mut files = self.files.write();

        if !files.contains_key(path) {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        files.retain(|p, _| !p.starts_with(path) || p == path);

        tracing::debug!(path = %path.display(), "removed all");
        Ok(())
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let mut files = self.files.write();

        let entry = files.remove(old_path).ok_or_else(|| FSError::NotFound {
            path: old_path.to_path_buf(),
        })?;

        if files.contains_key(new_path) {
            files.insert(old_path.to_path_buf(), entry);
            return Err(FSError::AlreadyExists {
                path: new_path.to_path_buf(),
            });
        }

        let mut new_entry = entry;
        new_entry.metadata.path = new_path.to_path_buf();

        files.insert(new_path.to_path_buf(), new_entry);

        tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed");
        Ok(())
    }
}
