use std::path::Path;

use async_trait::async_trait;
use chrono::Utc;
use racfs_core::{error::FSError, filesystem::ChmodFS};
use rusqlite::params;

use super::SqlFS;

#[async_trait]
impl ChmodFS for SqlFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let path_str = SqlFS::path_to_str(path)?;

        if !self.exists(&path_str)? {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        }

        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock();
        conn.execute(
            r#"
            UPDATE files
            SET mode = ?1, modified_at = ?2
            WHERE path = ?3
            "#,
            params![&mode, &now, &path_str],
        )
        .map_err(|e| FSError::Io {
            message: format!("Failed to chmod: {}", e),
        })?;

        tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
        Ok(())
    }
}
