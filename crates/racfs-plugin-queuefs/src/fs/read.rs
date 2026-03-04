use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::QueueFS;

#[async_trait]
impl ReadFS for QueueFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let (queue_name, rest) = Self::parse_queue_path(path)?;

        if rest.is_empty() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        self.ensure_queue_exists(&queue_name)?;

        let queue = self.get_queue(&queue_name)?;

        match rest.as_slice() {
            [h] if h == "head" => {
                let next = queue.get_next_unacknowledged();
                let data = next.unwrap_or("empty").to_string().into_bytes();
                Self::read_slice(&data, offset, size)
            }
            [t] if t == "tail" => {
                let data = Self::format_message_id(queue.get_next_id()).into_bytes();
                Self::read_slice(&data, offset, size)
            }
            [messages, id] if messages == "messages" => {
                let msg = queue.get_message(id)?;
                Self::read_slice(&msg.data, offset, size)
            }
            [metadata, count] if metadata == "metadata" && count == "count" => {
                let data = queue.get_count().to_string().into_bytes();
                Self::read_slice(&data, offset, size)
            }
            [metadata, config] if metadata == "metadata" && config == "config" => {
                let config = format!(
                    r#"{{"name":"{}","next_id":{},"head_id":{}}}"#,
                    queue_name,
                    queue.get_next_id(),
                    queue.head_id
                );
                Self::read_slice(config.as_bytes(), offset, size)
            }
            _ => Err(FSError::NotFound {
                path: path.to_path_buf(),
            }),
        }
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let (queue_name, rest) = Self::parse_queue_path(path)?;

        if rest.is_empty() {
            self.ensure_queue_exists(&queue_name)?;
            return Ok(FileMetadata::directory(path.to_path_buf()));
        }

        self.ensure_queue_exists(&queue_name)?;
        let queue = self.get_queue(&queue_name)?;

        match rest.as_slice() {
            [h] if h == "head" || h == "tail" => Ok(FileMetadata::file(path.to_path_buf(), 0)),
            [m, id] if m == "messages" => {
                let msg = queue.get_message(id)?;
                Ok(FileMetadata::file(
                    path.to_path_buf(),
                    msg.data.len() as u64,
                ))
            }
            [m, c] if m == "metadata" && c == "count" => {
                let count = queue.get_count();
                Ok(FileMetadata::file(
                    path.to_path_buf(),
                    count.to_string().len() as u64,
                ))
            }
            [m, cfg] if m == "metadata" && cfg == "config" => {
                let config = format!(
                    r#"{{"name":"{}","next_id":{},"head_id":{}}}"#,
                    queue_name,
                    queue.get_next_id(),
                    queue.head_id
                );
                Ok(FileMetadata::file(path.to_path_buf(), config.len() as u64))
            }
            _ => Err(FSError::NotFound {
                path: path.to_path_buf(),
            }),
        }
    }
}
