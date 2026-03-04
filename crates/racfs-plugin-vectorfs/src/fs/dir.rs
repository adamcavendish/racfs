use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::VectorFS;

#[async_trait]
impl DirFS for VectorFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let path_str = path.to_string_lossy();
        if !path_str.starts_with("/search/") {
            return Err(FSError::NotSupported {
                message: "Can only create directories under /search/".to_string(),
            });
        }

        let mut metadata = self.metadata.write();

        if metadata.contains_key(path) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let dir_meta = FileMetadata::directory(path.to_path_buf());
        metadata.insert(path.to_path_buf(), dir_meta);

        let query_path = path.join("query.txt");
        let matches_path = path.join("matches.txt");
        metadata.insert(query_path.clone(), FileMetadata::file(query_path, 0));
        metadata.insert(matches_path.clone(), FileMetadata::file(matches_path, 0));

        let query_id = path_str
            .trim_start_matches("/search/")
            .trim_end_matches('/');
        if !query_id.is_empty() && !query_id.contains('/') {
            self.search_queries
                .write()
                .insert(query_id.to_string(), Vec::new());
        }

        tracing::debug!(path = %path.display(), "created directory");
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let path_str = path.to_string_lossy();
        let prefix = if path_str == "/" {
            "".to_string()
        } else {
            path_str.to_string()
        };

        let mut entries: Vec<FileMetadata> = self
            .metadata
            .read()
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
                    Some(entry.clone())
                } else {
                    None
                }
            })
            .collect();

        if path_str == "/documents" {
            let doc_paths = self.list_document_paths().await?;
            for p in doc_paths {
                let _name = p.file_name().unwrap_or(p.as_os_str());
                let is_direct = p
                    .parent()
                    .map(|parent| parent == Path::new("/documents"))
                    .unwrap_or(false);
                if is_direct {
                    entries.push(FileMetadata::file(p.clone(), 0));
                }
            }
        }

        Ok(entries)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        if self.is_virtual_path(path) && path.to_string_lossy().starts_with("/index/") {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        let path_str = path.to_string_lossy();
        let mut removed = false;

        if path_str.starts_with("/documents/") {
            if self.get_document(path).await?.is_some() {
                self.delete_from_db(path_str.as_ref()).await?;
                removed = true;
            }
            self.xattrs.write().remove(path);
        }

        if path_str.starts_with("/search/") {
            let mut metadata = self.metadata.write();
            let has_children = metadata.keys().any(|p| p != path && p.starts_with(path));
            if has_children {
                return Err(FSError::DirectoryNotEmpty);
            }
            if metadata.remove(path).is_some() {
                removed = true;
            }
            let query_id = path_str
                .trim_start_matches("/search/")
                .trim_end_matches('/');
            if !query_id.is_empty() {
                let query_path = path.join("query.txt");
                let matches_path = path.join("matches.txt");
                metadata.remove(&query_path);
                metadata.remove(&matches_path);
                self.search_queries.write().remove(query_id);
            }
        }

        if !removed {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        if path_str.starts_with("/documents/") {
            self.metadata.write().remove(path);
        }

        tracing::debug!(path = %path.display(), "removed");
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        if self.is_virtual_path(path) && path.to_string_lossy().starts_with("/index/") {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        let path_str = path.to_string_lossy();
        if path_str.starts_with("/documents/") {
            let had_meta = self.metadata.read().contains_key(path);
            let paths = self.list_document_paths().await?;
            let to_remove: Vec<PathBuf> =
                paths.into_iter().filter(|p| p.starts_with(path)).collect();
            for p in &to_remove {
                let s = p.to_string_lossy();
                self.delete_from_db(s.as_ref()).await?;
                self.metadata.write().remove(p);
                self.xattrs.write().remove(p);
            }
            self.metadata.write().remove(path);
            if to_remove.is_empty() && !had_meta {
                return Err(FSError::NotFound {
                    path: path.to_path_buf(),
                });
            }
            tracing::debug!(path = %path.display(), "removed all");
            return Ok(());
        }

        let mut metadata = self.metadata.write();
        if !metadata.contains_key(path) {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }
        metadata.retain(|p, _| !p.starts_with(path) || p == path);
        Ok(())
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let old_str = old_path.to_string_lossy();
        let new_str = new_path.to_string_lossy();

        if old_str.starts_with("/index/") || new_str.starts_with("/index/") {
            return Err(FSError::PermissionDenied {
                path: old_path.to_path_buf(),
            });
        }

        if old_str.starts_with("/documents/") && new_str.starts_with("/documents/") {
            if old_path == new_path {
                return Ok(());
            }
            let Some((data, vector, mode, created_at, _)) = self.get_document(old_path).await?
            else {
                return Err(FSError::NotFound {
                    path: old_path.to_path_buf(),
                });
            };

            if self.get_document(new_path).await?.is_some() {
                self.delete_from_db(new_str.as_ref()).await?;
            }

            self.delete_from_db(old_str.as_ref()).await?;
            self.persist_document(
                &new_str,
                &data,
                &vector,
                mode,
                created_at,
                Some(chrono::Utc::now().timestamp_millis()),
            )
            .await?;

            self.metadata.write().remove(old_path);
            let mut meta =
                racfs_core::metadata::FileMetadata::file(new_path.to_path_buf(), data.len() as u64);
            meta.mode = mode;
            meta.created = created_at.and_then(chrono::DateTime::from_timestamp_millis);
            meta.modified =
                chrono::DateTime::from_timestamp_millis(chrono::Utc::now().timestamp_millis());
            meta.accessed = meta.modified;
            self.metadata.write().insert(new_path.to_path_buf(), meta);

            if let Some(xattr_map) = self.xattrs.write().remove(old_path) {
                self.xattrs
                    .write()
                    .insert(new_path.to_path_buf(), xattr_map);
            }

            tracing::debug!(old = %old_path.display(), new = %new_path.display(), "renamed document");
            return Ok(());
        }

        if old_str.starts_with("/search/") && new_str.starts_with("/search/") {
            if old_path == new_path {
                return Ok(());
            }
            let mut metadata = self.metadata.write();

            if !metadata.contains_key(old_path) {
                return Err(FSError::NotFound {
                    path: old_path.to_path_buf(),
                });
            }

            let new_prefix = if new_str.ends_with('/') {
                new_str.to_string()
            } else {
                format!("{}/", new_str)
            };
            let has_children = metadata.keys().any(|p| {
                let s = p.to_string_lossy();
                s.starts_with(&new_prefix) && s != new_prefix
            });
            if has_children {
                return Err(FSError::DirectoryNotEmpty);
            }

            if let Some(meta) = metadata.remove(old_path) {
                let mut new_meta = meta;
                new_meta.path = new_path.to_path_buf();
                metadata.insert(new_path.to_path_buf(), new_meta);
                tracing::debug!(old = %old_path.display(), new = %new_path.display(), "renamed search dir");
                return Ok(());
            }
        }

        Err(FSError::NotSupported {
            message: "rename only supported within /documents/ or within /search/".to_string(),
        })
    }
}
