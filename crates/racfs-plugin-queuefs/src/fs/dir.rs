use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::QueueFS;

#[async_trait]
impl DirFS for QueueFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let (queue_name, rest) = Self::parse_queue_path(path)?;

        if queue_name.is_empty() {
            return Err(FSError::InvalidInput {
                message: "invalid queue name".to_string(),
            });
        }

        if !rest.is_empty() {
            return Err(FSError::NotSupported {
                message: "nested directories not supported".to_string(),
            });
        }

        let mut queues = self.queues.write();
        if queues.contains_key(&queue_name) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let name = queue_name.clone();
        queues.insert(queue_name, super::Queue::new());
        tracing::debug!(queue = %name, "created queue");
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let (queue_name, rest) = Self::parse_queue_path(path)?;

        if rest.is_empty() && queue_name.is_empty() {
            let queues = self.queues.read();
            let entries: Vec<FileMetadata> = queues
                .keys()
                .map(|name| FileMetadata::directory(PathBuf::from(format!("/{}", name))))
                .collect();
            return Ok(entries);
        }

        if rest.is_empty() {
            self.ensure_queue_exists(&queue_name)?;
            let entries = vec![
                FileMetadata::file(PathBuf::from(format!("/{}/head", queue_name)), 0),
                FileMetadata::file(PathBuf::from(format!("/{}/tail", queue_name)), 0),
                FileMetadata::directory(PathBuf::from(format!("/{}/messages", queue_name))),
                FileMetadata::directory(PathBuf::from(format!("/{}/metadata", queue_name))),
                FileMetadata::directory(PathBuf::from(format!("/{}/.ack", queue_name))),
            ];
            return Ok(entries);
        }

        self.ensure_queue_exists(&queue_name)?;

        if rest.as_slice() == ["messages"] {
            let queue = self.get_queue(&queue_name)?;
            let entries: Vec<FileMetadata> = queue
                .messages
                .values()
                .filter(|m| !m.acknowledged)
                .map(|msg| {
                    FileMetadata::file(
                        PathBuf::from(format!("/{}/messages/{}", queue_name, msg.id)),
                        msg.data.len() as u64,
                    )
                })
                .collect();
            return Ok(entries);
        }

        if rest.as_slice() == ["metadata"] {
            let queue = self.get_queue(&queue_name)?;
            let count_data = queue.get_count().to_string().into_bytes();
            let config_data = format!(
                r#"{{"name":"{}","next_id":{},"head_id":{}}}"#,
                queue_name,
                queue.get_next_id(),
                queue.head_id
            )
            .into_bytes();
            let entries = vec![
                FileMetadata::file(
                    PathBuf::from(format!("/{}/metadata/count", queue_name)),
                    count_data.len() as u64,
                ),
                FileMetadata::file(
                    PathBuf::from(format!("/{}/metadata/config", queue_name)),
                    config_data.len() as u64,
                ),
            ];
            return Ok(entries);
        }

        if rest.as_slice() == [".ack"] {
            return Ok(vec![]);
        }

        Err(FSError::NotFound {
            path: path.to_path_buf(),
        })
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let (queue_name, rest) = Self::parse_queue_path(path)?;

        if rest.is_empty() {
            let mut queues = self.queues.write();
            if queues.remove(&queue_name).is_some() {
                tracing::debug!(queue = %queue_name, "removed queue");
                Ok(())
            } else {
                Err(FSError::NotFound {
                    path: path.to_path_buf(),
                })
            }
        } else {
            Err(FSError::NotSupported {
                message: "cannot remove queue components".to_string(),
            })
        }
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        self.remove(path).await
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "rename not supported in queuefs".to_string(),
        })
    }
}
