use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::{FileQuery, ProxyFS};

#[async_trait]
impl ReadFS for ProxyFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let url = format!("{}/api/v1/files", self.base_url);
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

        let bytes = text.into_bytes();
        let start = offset.max(0) as usize;
        let end = if size < 0 {
            bytes.len()
        } else {
            (offset + size).min(bytes.len() as i64) as usize
        };

        if start >= bytes.len() {
            return Ok(Vec::new());
        }

        Ok(bytes[start..end].to_vec())
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let url = format!("{}/api/v1/stat", self.base_url);
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

        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 5 {
            let mode: u32 = parts[1].parse().unwrap_or(0o644);
            let size: u64 = parts[2].parse().unwrap_or(0);
            let path_str = parts[4];
            Ok(FileMetadata {
                path: PathBuf::from(path_str),
                size,
                mode,
                created: None,
                modified: None,
                accessed: None,
                is_symlink: false,
                symlink_target: None,
            })
        } else {
            Err(FSError::Io {
                message: "Invalid stat response".to_string(),
            })
        }
    }
}
