use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::{MessageStorage, PathComponent, StreamFS};

#[async_trait]
impl WriteFS for StreamFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);

        match self.parse_path(&normalized) {
            Some(PathComponent::Message(stream_name, id)) => {
                let mut state = self.state.write();
                let stream =
                    state
                        .streams
                        .get_mut(&stream_name)
                        .ok_or_else(|| FSError::NotFound {
                            path: normalized.clone(),
                        })?;

                stream.messages.insert(id, MessageStorage::Raw(Vec::new()));
                if id >= stream.tail {
                    stream.tail = id + 1;
                }
                Ok(())
            }
            Some(PathComponent::Head(stream_name)) => {
                let mut state = self.state.write();
                let stream =
                    state
                        .streams
                        .get_mut(&stream_name)
                        .ok_or_else(|| FSError::NotFound {
                            path: normalized.clone(),
                        })?;
                stream.head = 1;
                Ok(())
            }
            _ => Err(FSError::PermissionDenied { path: normalized }),
        }
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let normalized = self.normalize_path(path);
        let mut state = self.state.write();

        match self.parse_path(&normalized) {
            Some(PathComponent::Message(stream_name, id)) => {
                let stream =
                    state
                        .streams
                        .get_mut(&stream_name)
                        .ok_or_else(|| FSError::NotFound {
                            path: normalized.clone(),
                        })?;

                let existing = stream
                    .messages
                    .get(&id)
                    .map(|s| self.message_storage_to_bytes(s));
                let mut msg = match existing {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(e)) => return Err(e),
                    None => Vec::new(),
                };

                let offset = offset as usize;
                if offset > msg.len() {
                    msg.resize(offset, 0);
                }

                let end = offset + data.len();
                if end > msg.len() {
                    msg.resize(end, 0);
                }
                msg[offset..end].copy_from_slice(data);

                let stored = self.bytes_to_message_storage(&msg)?;
                stream.messages.insert(id, stored);

                if id >= stream.tail {
                    stream.tail = id + 1;
                }

                while stream.messages.len() > self.config.buffer_size {
                    if let Some((&first_key, _)) = stream.messages.iter().next() {
                        stream.messages.remove(&first_key);
                        stream.head = first_key + 1;
                    }
                }

                Ok(data.len() as u64)
            }
            Some(PathComponent::Tail(stream_name)) => {
                let stream =
                    state
                        .streams
                        .get_mut(&stream_name)
                        .ok_or_else(|| FSError::NotFound {
                            path: normalized.clone(),
                        })?;

                let id = stream.tail;
                let stored = self.bytes_to_message_storage(data)?;
                stream.messages.insert(id, stored);
                stream.tail = id + 1;

                while stream.messages.len() > self.config.buffer_size {
                    if let Some((&first_key, _)) = stream.messages.iter().next() {
                        stream.messages.remove(&first_key);
                        stream.head = first_key + 1;
                    }
                }

                Ok(data.len() as u64)
            }
            Some(PathComponent::Head(stream_name)) => {
                let stream =
                    state
                        .streams
                        .get_mut(&stream_name)
                        .ok_or_else(|| FSError::NotFound {
                            path: normalized.clone(),
                        })?;

                let text = String::from_utf8_lossy(data);
                if let Ok(new_head) = text.trim().parse::<u64>() {
                    stream.head = new_head;
                }
                Ok(data.len() as u64)
            }
            _ => Err(FSError::PermissionDenied { path: normalized }),
        }
    }
}
