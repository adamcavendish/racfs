use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags, metadata::FileMetadata};

use super::{FileEntry, MemFS};

#[async_trait]
impl WriteFS for MemFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        self.ensure_parent_exists(path)?;

        let mut files = self.files.write();

        if files.contains_key(path) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let metadata = FileMetadata::file(path.to_path_buf(), 0);
        files.insert(
            path.to_path_buf(),
            FileEntry {
                data: Vec::new(),
                metadata,
                is_symlink: false,
                symlink_target: None,
            },
        );

        tracing::debug!(path = %path.display(), "created file");
        self.inc_op("create");
        self.inc_entries(1);
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let mut files = self.files.write();

        let entry = files.get_mut(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let _is_truncate = flags.contains(WriteFlags::from_bits_truncate(0x0020));
        let new_size = if flags.contains_append() || offset as usize >= entry.data.len() {
            // Append mode or write past end
            let end = offset.max(0) as usize;
            if end > entry.data.len() {
                entry.data.resize(end, 0);
            }
            entry.data.extend_from_slice(data);
            entry.data.len()
        } else if offset <= 0 {
            // Truncate mode or write from start (only when offset is 0)
            entry.data = data.to_vec();
            data.len()
        } else {
            // Write in the middle
            let start = offset as usize;
            if start > entry.data.len() {
                entry.data.resize(start, 0);
            }
            if start + data.len() > entry.data.len() {
                entry.data.resize(start + data.len(), 0);
            }
            entry.data[start..start + data.len()].copy_from_slice(data);
            entry.data.len()
        };

        entry.metadata.size = new_size as u64;
        entry.metadata.modified = Some(Utc::now());

        tracing::debug!(path = %path.display(), bytes = data.len(), "wrote");
        self.inc_op("write");
        Ok(data.len() as u64)
    }
}
