use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::PgsFS;

#[async_trait]
impl ChmodFS for PgsFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let path_str = PgsFS::path_to_str(path)?;

        if !self.exists(&path_str).await? {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let now = Utc::now().timestamp_millis();
        sqlx::query(
            r#"
            UPDATE files
            SET mode = $1, modified_at = $2
            WHERE path = $3
            "#,
        )
        .bind(&(mode as i32))
        .bind(&now)
        .bind(&path_str)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| FSError::Io {
            message: format!("Failed to chmod: {}", e),
        })?;

        tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
        Ok(())
    }
}
