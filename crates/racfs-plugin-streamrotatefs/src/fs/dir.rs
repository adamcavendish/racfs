use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::StreamRotateFS;

#[async_trait]
impl DirFS for StreamRotateFS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        match path_str.as_str() {
            "/" | "/archive" => Ok(()),
            _ => Err(FSError::AlreadyExists { path: normalized }),
        }
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();
        let state = self.state.read();

        match path_str.as_str() {
            "/" => {
                let entries = vec![
                    FileMetadata::file(PathBuf::from("/current"), state.current.data.len() as u64),
                    FileMetadata::directory(PathBuf::from("/archive")),
                    FileMetadata::file(PathBuf::from("/rotate"), 0),
                    FileMetadata::file(PathBuf::from("/config"), self.config_string().len() as u64),
                ];
                Ok(entries)
            }
            "/archive" => {
                let entries: Vec<FileMetadata> = state
                    .archive
                    .iter()
                    .map(|(seq, entry)| {
                        let filename = format!("{:03}.log", seq);
                        let path = PathBuf::from("/archive").join(&filename);
                        let mut metadata = entry.metadata.clone();
                        metadata.path = path;
                        metadata
                    })
                    .collect();
                Ok(entries)
            }
            _ => Err(FSError::NotADirectory { path: normalized }),
        }
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        match path_str.as_str() {
            "/" | "/current" | "/rotate" | "/config" | "/archive" => {
                Err(FSError::PermissionDenied { path: normalized })
            }
            path_str_ref if path_str_ref.starts_with("/archive/") => {
                let name = path_str_ref.strip_prefix("/archive/").unwrap_or("");
                let seq_str = name.strip_suffix(".log").unwrap_or(name);

                if let Ok(seq) = seq_str.parse::<usize>() {
                    let mut state = self.state.write();
                    if state.archive.remove(&seq).is_some() {
                        tracing::debug!(seq = seq, "removed archive file");
                        return Ok(());
                    }
                }
                Err(FSError::NotFound { path: normalized })
            }
            _ => Err(FSError::NotFound { path: normalized }),
        }
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        match path_str.as_str() {
            "/archive" => {
                let mut state = self.state.write();
                state.archive.clear();
                tracing::debug!("cleared archive");
                Ok(())
            }
            "/" | "/current" | "/rotate" | "/config" => {
                Err(FSError::PermissionDenied { path: normalized })
            }
            path_str_ref if path_str_ref.starts_with("/archive/") => self.remove(path).await,
            _ => Err(FSError::NotFound { path: normalized }),
        }
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::NotSupported {
            message: "rename not supported".to_string(),
        })
    }
}
