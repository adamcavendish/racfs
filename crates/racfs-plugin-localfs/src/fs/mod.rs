//! Local filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::FileSystem};

/// Local filesystem wrapper.
pub struct LocalFS {
    root: PathBuf,
}

impl LocalFS {
    /// Create a new local filesystem wrapper.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Get the root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a path relative to the root.
    pub(super) fn resolve(&self, path: &Path) -> Result<PathBuf, FSError> {
        // Strip leading "/" from path to make it relative
        let relative = path.strip_prefix("/").unwrap_or(path);
        let resolved = self.root.join(relative);

        // Security: prevent path traversal by checking for ".." components
        // and verifying the resolved path starts with root
        let mut normalized = PathBuf::new();
        for component in resolved.components() {
            match component {
                std::path::Component::ParentDir => {
                    // Check if we're about to go above root
                    if !normalized.starts_with(&self.root) {
                        return Err(FSError::PermissionDenied {
                            path: path.to_path_buf(),
                        });
                    }
                    // Pop the last component
                    if normalized.pop() {
                        // Continue
                    } else {
                        normalized.push(component);
                    }
                }
                _ => {
                    normalized.push(component);
                }
            }
        }

        // Final check: ensure path is under root
        if !normalized.starts_with(&self.root) {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        Ok(normalized)
    }
}

#[async_trait]
impl FileSystem for LocalFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&resolved)
                .map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?;

            file.set_len(size).map_err(|e| FSError::Io {
                message: e.to_string(),
            })?;
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let resolved = self.resolve(path)?;

        tokio::task::spawn_blocking(move || {
            if !resolved.exists() {
                std::fs::File::create(&resolved).map_err(|e| FSError::Io {
                    message: e.to_string(),
                })?;
            }
            Ok(())
        })
        .await
        .map_err(|e| FSError::Io {
            message: format!("spawn_blocking join error: {}", e),
        })?
    }
}
