use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};
use rusqlite::params;

use super::KvFS;

#[async_trait]
impl ChmodFS for KvFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        match &self.backend {
            super::KvBackend::Memory(_store, metadata) => {
                let mut metadata = metadata.write();

                let meta = metadata.get_mut(path).ok_or_else(|| FSError::NotFound {
                    path: path.to_path_buf(),
                })?;

                meta.set_permissions(mode);
                tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
                Ok(())
            }
            super::KvBackend::Sqlite(conn) => {
                let path_str = KvFS::path_to_str(path)?;
                let conn = conn.lock();
                let current_mode: u32 = conn
                    .query_row(
                        "SELECT mode FROM kvfs WHERE path = ?1",
                        params![&path_str],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(|_| FSError::NotFound {
                        path: path.to_path_buf(),
                    })? as u32;
                let new_mode = (current_mode & 0o170000) | (mode & 0o777);
                conn.execute(
                    "UPDATE kvfs SET mode = ?1 WHERE path = ?2",
                    params![new_mode as i64, &path_str],
                )
                .map_err(|e| FSError::Io {
                    message: format!("Failed to chmod: {}", e),
                })?;
                tracing::debug!(path = %path.display(), mode = format!("{:o}", mode), "chmod");
                Ok(())
            }
        }
    }
}
