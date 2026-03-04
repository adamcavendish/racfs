use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::{FileEntry, MemFS};

#[async_trait]
impl DirFS for MemFS {
    async fn mkdir(&self, path: &Path, perm: u32) -> Result<(), FSError> {
        if let Some(parent) = path.parent()
            && parent.as_os_str() != "/"
        {
            self.ensure_parent_exists(parent)?;
        }

        let mut files = self.files.write();

        if files.contains_key(path) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let mut metadata = FileMetadata::directory(path.to_path_buf());
        metadata.set_permissions(perm);
        files.insert(
            path.to_path_buf(),
            FileEntry {
                data: Vec::new(),
                metadata,
                is_symlink: false,
                symlink_target: None,
            },
        );

        tracing::debug!(path = %path.display(), "created directory");
        self.inc_op("mkdir");
        self.inc_entries(1);
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

                // Only include direct children
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

        self.inc_op("readdir");
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

        self.xattrs.write().remove(path);

        tracing::debug!(path = %path.display(), "removed");
        self.inc_op("remove");
        self.inc_entries(-1);
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let mut files = self.files.write();

        if !files.contains_key(path) {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let removed = files
            .keys()
            .filter(|p| *p == path || p.starts_with(path))
            .count();
        files.retain(|p, _| *p != path && !p.starts_with(path));

        let mut xattrs = self.xattrs.write();
        xattrs.retain(|p, _| *p != path && !p.starts_with(path));

        tracing::debug!(path = %path.display(), "removed all");
        self.inc_op("remove_all");
        self.inc_entries(-(removed as i64));
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

        {
            let mut x = self.xattrs.write();
            if let Some(attrs) = x.remove(old_path) {
                x.insert(new_path.to_path_buf(), attrs);
            }
        }

        tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed");
        self.inc_op("rename");
        Ok(())
    }
}
