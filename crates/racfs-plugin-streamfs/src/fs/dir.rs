use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::{PathComponent, StreamFS};

#[async_trait]
impl DirFS for StreamFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);

        match self.parse_path(&normalized) {
            Some(PathComponent::StreamsDir) => Ok(()),
            Some(PathComponent::StreamDir(name)) => self.create_stream(&name),
            Some(PathComponent::DataDir(stream_name)) => {
                let state = self.state.read();
                if state.streams.contains_key(&stream_name) {
                    Ok(())
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            _ => Err(FSError::PermissionDenied { path: normalized }),
        }
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let normalized = self.normalize_path(path);
        let state = self.state.read();

        match self.parse_path(&normalized) {
            Some(PathComponent::Root) => {
                Ok(vec![FileMetadata::directory(PathBuf::from("/streams"))])
            }
            Some(PathComponent::StreamsDir) => {
                let mut entries: Vec<FileMetadata> = state
                    .streams
                    .keys()
                    .map(|name| {
                        FileMetadata::directory(PathBuf::from(format!("/streams/{}", name)))
                    })
                    .collect();
                entries.sort_by(|a, b| a.path.cmp(&b.path));
                Ok(entries)
            }
            Some(PathComponent::StreamDir(stream_name)) => {
                let _stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;

                Ok(vec![
                    FileMetadata::directory(PathBuf::from(format!(
                        "/streams/{}/data",
                        stream_name
                    ))),
                    FileMetadata::file(PathBuf::from(format!("/streams/{}/head", stream_name)), 6),
                    FileMetadata::file(PathBuf::from(format!("/streams/{}/tail", stream_name)), 6),
                    FileMetadata::file(
                        PathBuf::from(format!("/streams/{}/config", stream_name)),
                        0,
                    ),
                ])
            }
            Some(PathComponent::DataDir(stream_name)) => {
                let stream = state
                    .streams
                    .get(&stream_name)
                    .ok_or_else(|| FSError::NotFound {
                        path: normalized.clone(),
                    })?;

                let mut entries: Vec<FileMetadata> = stream
                    .messages
                    .iter()
                    .map(|(&id, stored)| {
                        let path =
                            PathBuf::from(format!("/streams/{}/data/{:06}.msg", stream_name, id));
                        FileMetadata::file(path, stored.len() as u64)
                    })
                    .collect();
                entries.sort_by(|a, b| a.path.cmp(&b.path));
                Ok(entries)
            }
            _ => Err(FSError::NotADirectory { path: normalized }),
        }
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
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

                if stream.messages.remove(&id).is_some() {
                    Ok(())
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            Some(PathComponent::StreamDir(name)) => {
                let mut state = self.state.write();
                if state.streams.remove(&name).is_some() {
                    Ok(())
                } else {
                    Err(FSError::NotFound { path: normalized })
                }
            }
            _ => Err(FSError::PermissionDenied { path: normalized }),
        }
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        self.remove(path).await
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: PathBuf::from("/"),
        })
    }
}
