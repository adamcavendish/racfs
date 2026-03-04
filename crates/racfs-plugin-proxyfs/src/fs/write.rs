use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::WriteFS, flags::WriteFlags};

use super::{ProxyFS, WriteRequest};

#[async_trait]
impl WriteFS for ProxyFS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        let url = format!("{}/api/v1/files", self.base_url);
        let query = super::FileQuery {
            path: path.to_string_lossy().to_string(),
        };

        self.client
            .post(&url)
            .query(&query)
            .send()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        tracing::debug!(path = %path.display(), "created via proxy");
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        let url = format!("{}/api/v1/files", self.base_url);
        let body = WriteRequest {
            path: path.to_string_lossy().to_string(),
            data: String::from_utf8_lossy(data).to_string(),
            offset: Some(offset),
        };

        self.client
            .put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        tracing::debug!(path = %path.display(), bytes = data.len(), "wrote via proxy");
        Ok(data.len() as u64)
    }
}
