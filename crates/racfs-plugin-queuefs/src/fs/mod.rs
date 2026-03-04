//! Message queue filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use parking_lot::RwLock;
use racfs_core::{error::FSError, filesystem::FileSystem};

/// Message queue filesystem with POSIX-compliant operations.
///
/// Directory structure:
/// ```text
/// /{queue_name}/
/// |-- head        # Read: get next message ID (or "empty")
/// |-- tail        # Write: append message, returns ID; Read: get tail ID
/// |-- messages/
/// |   |-- 000001  # Message content (read-only after creation)
/// |   |-- 000002
/// |   |-- ...
/// |-- metadata/
/// |   |-- count   # Number of messages
/// |   |-- config  # Queue configuration (read-only)
/// |-- .ack/
///     |-- {id}    # Write "done" to acknowledge/remove message
/// ```
pub struct QueueFS {
    pub(crate) queues: Arc<RwLock<HashMap<String, Queue>>>,
}

/// Represents a single message queue.
#[derive(Clone)]
pub(crate) struct Queue {
    pub(crate) next_id: u64,
    pub(crate) head_id: u64,
    pub(crate) messages: BTreeMap<String, Message>,
}

/// A single message in the queue.
#[derive(Clone)]
pub(crate) struct Message {
    pub(crate) id: String,
    pub(crate) data: Vec<u8>,
    pub(crate) acknowledged: bool,
}

impl Queue {
    fn new() -> Self {
        Self {
            next_id: 1,
            head_id: 1,
            messages: BTreeMap::new(),
        }
    }

    fn next_message_id(&mut self) -> String {
        let id = self.next_id;
        self.next_id += 1;
        format!("{:06}", id)
    }

    fn enqueue(&mut self, data: Vec<u8>) -> String {
        let id = self.next_message_id();
        self.messages.insert(
            id.clone(),
            Message {
                id: id.clone(),
                data,
                acknowledged: false,
            },
        );
        id
    }

    fn dequeue(&mut self, id: &str) -> Result<(), FSError> {
        let msg = self.messages.get_mut(id).ok_or_else(|| FSError::NotFound {
            path: PathBuf::from(id),
        })?;
        msg.acknowledged = true;
        Ok(())
    }

    fn get_message(&self, id: &str) -> Result<&Message, FSError> {
        self.messages
            .get(id)
            .filter(|m| !m.acknowledged)
            .ok_or_else(|| FSError::NotFound {
                path: PathBuf::from(id),
            })
    }

    fn get_next_unacknowledged(&self) -> Option<&str> {
        for (id, msg) in self.messages.iter() {
            if !msg.acknowledged {
                return Some(id);
            }
        }
        None
    }

    fn get_count(&self) -> u64 {
        self.messages.values().filter(|m| !m.acknowledged).count() as u64
    }

    fn get_next_id(&self) -> u64 {
        self.next_id
    }
}

impl QueueFS {
    /// Create a new queue filesystem.
    pub fn new() -> Self {
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Parse a queue path to extract queue name and components.
    ///
    /// Returns (queue_name, rest_path) or None if not a queue path.
    pub(crate) fn parse_queue_path(path: &Path) -> Result<(String, Vec<String>), FSError> {
        let components: Vec<&str> = path.iter().filter_map(|c| c.to_str()).collect();

        let start = if components
            .first()
            .is_some_and(|c| *c == "/" || c.is_empty())
        {
            1
        } else {
            0
        };

        if start >= components.len() {
            return Ok(("".to_string(), vec![]));
        }

        let queue_name = components[start].to_string();
        let rest: Vec<String> = components[start + 1..]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Ok((queue_name, rest))
    }

    /// Format a message ID as a 6-digit zero-padded string.
    pub(crate) fn format_message_id(id: u64) -> String {
        format!("{:06}", id)
    }

    /// Get or create a queue by name.
    pub(crate) fn get_queue(&self, name: &str) -> Result<Queue, FSError> {
        let queues = self.queues.read();
        queues.get(name).cloned().ok_or_else(|| FSError::NotFound {
            path: PathBuf::from(format!("/{}", name)),
        })
    }

    pub(crate) fn ensure_queue_exists(&self, name: &str) -> Result<(), FSError> {
        let queues = self.queues.read();
        if !queues.contains_key(name) {
            return Err(FSError::NotFound {
                path: PathBuf::from(format!("/{}", name)),
            });
        }
        Ok(())
    }

    pub(crate) fn read_slice(data: &[u8], offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let data_len = data.len();
        let start = offset.max(0) as usize;
        let end = if size < 0 {
            data_len
        } else {
            (offset + size).min(data_len as i64) as usize
        };

        if start >= data_len {
            return Ok(Vec::new());
        }

        Ok(data[start..end].to_vec())
    }
}

impl Default for QueueFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for QueueFS {
    async fn truncate(&self, _path: &Path, _size: u64) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "truncate not supported in queuefs".to_string(),
        })
    }

    async fn touch(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "touch not supported in queuefs".to_string(),
        })
    }
}
