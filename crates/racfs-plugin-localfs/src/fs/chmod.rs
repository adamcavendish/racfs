use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ChmodFS};

use super::LocalFS;

#[async_trait]
impl ChmodFS for LocalFS {
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&resolved, std::fs::Permissions::from_mode(mode))
                    .map_err(|e| FSError::Io {
                        message: e.to_string(),
                    })?;
            }
            #[cfg(not(unix))]
            {
                let _ = (resolved, mode);
                return Err(FSError::NotSupported {
                    message: "chmod not supported on this platform".to_string(),
                });
            }
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }
}
