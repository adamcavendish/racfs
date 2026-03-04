use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::{PathComponent, StreamFS};

#[async_trait]
impl ReadFS for StreamFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let normalized = self.normalize_path(path);
        let state = self.state.read();

        match self.parse_path(&normalized) {
            Some(PathComponent::Message(stream_name, id)) => {
                let stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;

                let stored = stream.messages.get(&id).ok_or_else(|| FSError::NotFound {
                    path: normalized.clone(),
                })?;

                let data = self.message_storage_to_bytes(stored)?;
                let start = offset.max(0) as usize;
                let end = if size < 0 {
                    data.len()
                } else {
                    std::cmp::min(offset as usize + size as usize, data.len())
                };

                if start >= data.len() {
                    return Ok(Vec::new());
                }

                Ok(data[start..end].to_vec())
            }
            Some(PathComponent::Head(stream_name)) => {
                let stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;
                Ok(format!("{:06}", stream.head).into_bytes())
            }
            Some(PathComponent::Tail(stream_name)) => {
                let stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;
                Ok(format!("{:06}", stream.tail).into_bytes())
            }
            Some(PathComponent::Config(stream_name)) => {
                let stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;
                let config = format!(
                    "name={}\nbuffer_size={}\nhead={}\ntail={}\n",
                    stream.name, self.config.buffer_size, stream.head, stream.tail
                );
                Ok(config.into_bytes())
            }
            _ => Err(FSError::NotFound { path: normalized }),
        }
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let normalized = self.normalize_path(path);
        let state = self.state.read();

        match self.parse_path(&normalized) {
            Some(PathComponent::Root) => Ok(FileMetadata::directory(PathBuf::from("/"))),
            Some(PathComponent::StreamsDir) => {
                Ok(FileMetadata::directory(PathBuf::from("/streams")))
            }
            Some(PathComponent::StreamDir(stream_name)) => {
                if state.streams.contains_key(&stream_name) {
                    Ok(FileMetadata::directory(PathBuf::from(format!(
                        "/streams/{}",
                        stream_name
                    ))))
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            Some(PathComponent::DataDir(stream_name)) => {
                if state.streams.contains_key(&stream_name) {
                    Ok(FileMetadata::directory(PathBuf::from(format!(
                        "/streams/{}/data",
                        stream_name
                    ))))
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            Some(PathComponent::Message(stream_name, id)) => {
                let stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;

                let stored = stream.messages.get(&id).ok_or_else(|| FSError::NotFound {
                    path: normalized.clone(),
                })?;

                Ok(FileMetadata::file(
                    PathBuf::from(format!("/streams/{}/data/{:06}.msg", stream_name, id)),
                    stored.len() as u64,
                ))
            }
            Some(PathComponent::Head(stream_name)) => {
                if state.streams.contains_key(&stream_name) {
                    Ok(FileMetadata::file(
                        PathBuf::from(format!("/streams/{}/head", stream_name)),
                        6,
                    ))
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            Some(PathComponent::Tail(stream_name)) => {
                if state.streams.contains_key(&stream_name) {
                    Ok(FileMetadata::file(
                        PathBuf::from(format!("/streams/{}/tail", stream_name)),
                        6,
                    ))
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            Some(PathComponent::Config(stream_name)) => {
                if state.streams.contains_key(&stream_name) {
                    Ok(FileMetadata::file(
                        PathBuf::from(format!("/streams/{}/config", stream_name)),
                        0,
                    ))
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            None => Err(FSError::NotFound { path: normalized }),
        }
    }
}
