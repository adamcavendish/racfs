use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::QueueFS;

#[async_trait]
impl WriteFS for QueueFS {
    async fn create(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "create not supported in queuefs".to_string(),
        })
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let (queue_name, rest) = Self::parse_queue_path(path)?;

        if rest.is_empty() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        self.ensure_queue_exists(&queue_name)?;

        match rest.as_slice() {
            [t] if t == "tail" => {
                let mut queues = self.queues.write();
                let queue = queues
                    .get_mut(&queue_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: std::path::PathBuf::from(format!("/{}", queue_name)),
                    })?;
                let id = queue.enqueue(data.to_vec());
                tracing::debug!(queue = %queue_name, id = %id, bytes = data.len(), "enqueued message");
                Ok(id.len() as u64)
            }
            [ack, id] if ack == ".ack" => {
                if offset != 0 || !flags.is_empty() {
                    return Err(FSError::InvalidInput {
                        message: "offset and flags not supported for ack".to_string(),
                    });
                }
                let data_str = std::str::from_utf8(data).map_err(|e| FSError::InvalidUtf8 {
                    message: e.to_string(),
                })?;
                if data_str.trim() != "done" {
                    return Err(FSError::InvalidInput {
                        message: "must write 'done' to acknowledge".to_string(),
                    });
                }
                let mut queues = self.queues.write();
                let queue = queues
                    .get_mut(&queue_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: std::path::PathBuf::from(format!("/{}", queue_name)),
                    })?;
                queue.dequeue(id)?;
                tracing::debug!(queue = %queue_name, id = %id, "acknowledged message");
                Ok(data.len() as u64)
            }
            _ => Err(FSError::NotSupported {
                message: "write only supported on tail and .ack paths".to_string(),
            }),
        }
    }
}
