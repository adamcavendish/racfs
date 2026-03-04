use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::{CreateDirectoryRequest, FileQuery, ProxyFS};

#[async_trait]
impl DirFS for ProxyFS {
    async fn mkdir(&self, path: &Path, perm: u32) -> Result<(), FSError> {
        let url = format!("{}/api/v1/directories", self.base_url);
        let body = CreateDirectoryRequest {
            path: path.to_string_lossy().to_string(),
            perm: Some(perm),
        };

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        tracing::debug!(path = %path.display(), "created directory via proxy");
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        let url = format!("{}/api/v1/directories", self.base_url);
        let query = FileQuery {
            path: path.to_string_lossy().to_string(),
        };

        let response =
            self.client
                .get(&url)
                .query(&query)
                .send()
                .await
                .map_err(|e: reqwest::Error| FSError::Io {
                    message: e.to_string(),
                })?;

        let text = response
            .text()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        let entries: Vec<FileMetadata> = text
            .lines()
            .filter(|line: &&str| !line.is_empty())
            .map(|line: &str| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let mode: u32 = parts[1].parse().unwrap_or(0o644);
                    let size: u64 = parts[2].parse().unwrap_or(0);
                    let path_str = parts[3];
                    FileMetadata {
                        path: PathBuf::from(path_str),
                        size,
                        mode,
                        created: None,
                        modified: None,
                        accessed: None,
                        is_symlink: false,
                        symlink_target: None,
                    }
                } else {
                    FileMetadata::file(PathBuf::from(line), 0)
                }
            })
            .collect();

        Ok(entries)
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        let url = format!("{}/api/v1/files", self.base_url);
        let query = FileQuery {
            path: path.to_string_lossy().to_string(),
        };

        self.client
            .delete(&url)
            .query(&query)
            .send()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        tracing::debug!(path = %path.display(), "removed via proxy");
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        self.remove(path).await
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        let url = format!("{}/api/v1/rename", self.base_url);
        let body = super::RenameRequest {
            old_path: old_path.to_string_lossy().to_string(),
            new_path: new_path.to_string_lossy().to_string(),
        };

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        tracing::debug!(old_path = %old_path.display(), new_path = %new_path.display(), "renamed via proxy");
        Ok(())
    }
}
