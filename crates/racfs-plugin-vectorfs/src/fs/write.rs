use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::VectorFS;

#[async_trait]
impl WriteFS for VectorFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        if self.is_virtual_path(path) {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        let path_str = path.to_string_lossy();
        if !path_str.starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "Can only create files in /documents/".to_string(),
            });
        }

        if self.get_document(path).await?.is_some() {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let now = Utc::now().timestamp_millis();
        let empty_vector = self.content_to_vector(&[]);
        self.persist_document(&path_str, &[], &empty_vector, 0o644, Some(now), Some(now))
            .await?;

        let file_meta = racfs_core::metadata::FileMetadata::file(path.to_path_buf(), 0);
        self.metadata.write().insert(path.to_path_buf(), file_meta);

        tracing::debug!(path = %path.display(), "created document");
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let path_str = path.to_string_lossy();

        if path_str.contains("/search/") && path_str.ends_with("/query.txt") {
            let query_id = path_str
                .trim_start_matches("/search/")
                .trim_end_matches("/query.txt")
                .trim_end_matches('/');
            if query_id.is_empty() || query_id.contains('/') {
                return Err(FSError::PermissionDenied {
                    path: path.to_path_buf(),
                });
            }
            let mut queries = self.search_queries.write();
            let stored = queries.get_mut(query_id).ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })?;
            if offset <= 0 && !flags.contains_append() {
                *stored = data.to_vec();
            } else if flags.contains_append() || offset as usize >= stored.len() {
                stored.extend_from_slice(data);
            } else {
                let start = offset as usize;
                let end = start + data.len();
                if end > stored.len() {
                    stored.resize(end, 0);
                }
                stored[start..end].copy_from_slice(data);
            }
            let len = stored.len();
            drop(queries);
            if let Some(m) = self.metadata.write().get_mut(path) {
                m.size = len as u64;
            }
            return Ok(data.len() as u64);
        }

        if self.is_virtual_path(path) {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        if !path_str.starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "Can only write to files in /documents/".to_string(),
            });
        }

        let Some((mut existing_data, _, mode, created_at, _)) = self.get_document(path).await?
        else {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        };

        let new_data = if flags.contains_append() || offset as usize >= existing_data.len() {
            let start = offset.max(0) as usize;
            if start > existing_data.len() {
                existing_data.resize(start, 0);
            }
            existing_data.extend_from_slice(data);
            existing_data
        } else if offset <= 0
            || flags.contains(WriteFlags::from_bits_truncate(0x0020 /* O_TRUNC */))
        {
            data.to_vec()
        } else {
            let start = offset as usize;
            if start > existing_data.len() {
                existing_data.resize(start, 0);
            }
            let end = start + data.len();
            if end > existing_data.len() {
                existing_data.resize(end, 0);
            }
            existing_data[start..end].copy_from_slice(data);
            existing_data
        };

        let vector = {
            let maybe_vector_bytes = self
                .xattrs
                .read()
                .get(path)
                .and_then(|m| m.get("racfs.vector").cloned());
            if let Some(ref value) = maybe_vector_bytes {
                if let Ok(v) = Self::parse_vector_xattr(value, self.config.dimension) {
                    v
                } else {
                    self.embed_content(&new_data).await?
                }
            } else {
                self.embed_content(&new_data).await?
            }
        };

        let modified_at = Utc::now().timestamp_millis();
        self.persist_document(&path_str, &new_data, &vector, mode, created_at, Some(modified_at))
            .await?;

        if let Some(m) = self.metadata.write().get_mut(path) {
            m.size = new_data.len() as u64;
            m.modified = chrono::DateTime::from_timestamp_millis(modified_at);
        }
        let count = self.document_count().await?;
        if let Some(m) = self
            .metadata
            .write()
            .get_mut(&PathBuf::from("/index/count"))
        {
            m.size = count as u64;
        }

        tracing::debug!(path = %path.display(), bytes = data.len(), "wrote document");
        Ok(data.len() as u64)
    }
}
