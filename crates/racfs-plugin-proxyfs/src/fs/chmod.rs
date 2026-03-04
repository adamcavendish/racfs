use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::{ChmodRequest, ProxyFS};

#[async_trait]
impl ChmodFS for ProxyFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let url = format!("{}/api/v1/chmod", self.base_url);
        let body = ChmodRequest {
            path: path.to_string_lossy().to_string(),
            mode,
        };

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e: reqwest::Error| FSError::Io {
                message: e.to_string(),
            })?;

        tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod via proxy");
        Ok(())
    }
}
