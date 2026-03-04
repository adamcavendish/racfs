//! StreamRotateFS filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use racfs_core::{error::FSError, filesystem::FileSystem, metadata::FileMetadata};

/// Configuration for StreamRotateFS.
#[derive(Debug, Clone)]
pub struct RotateConfig {
    /// Maximum file size before rotation (default: 1MB)
    pub max_size: usize,
    /// Number of files to keep in archive (default: 10)
    pub max_files: usize,
    /// Whether to gzip old files (not yet implemented, just naming)
    pub compress: bool,
    /// Base path for persistent storage (optional, not used yet)
    pub base_path: PathBuf,
}

impl Default for RotateConfig {
    fn default() -> Self {
        Self {
            max_size: 1024 * 1024,
            max_files: 10,
            compress: false,
            base_path: PathBuf::new(),
        }
    }
}

/// Rotating log files filesystem.
///
/// Provides a virtual filesystem with automatic log rotation:
/// - `/current` - Active log file (append-only)
/// - `/archive/` - Rotated log files
/// - `/rotate` - Write "rotate" to force manual rotation
/// - `/config` - Read-only configuration
pub struct StreamRotateFS {
    pub(super) config: Arc<RotateConfig>,
    pub(super) state: Arc<RwLock<RotateState>>,
}

#[derive(Debug, Clone)]
pub(super) struct FileEntry {
    pub(super) data: Vec<u8>,
    pub(super) metadata: FileMetadata,
}

#[derive(Debug)]
pub(super) struct RotateState {
    pub(super) current: FileEntry,
    pub(super) archive: BTreeMap<usize, FileEntry>,
    pub(super) next_seq: usize,
}

impl FileEntry {
    pub(super) fn file(path: PathBuf, content: Vec<u8>) -> Self {
        let size = content.len() as u64;
        Self {
            data: content,
            metadata: FileMetadata::file(path, size),
        }
    }

    pub(super) fn dir(path: PathBuf) -> Self {
        Self {
            data: Vec::new(),
            metadata: FileMetadata::directory(path),
        }
    }

    pub(super) fn is_file(&self) -> bool {
        self.metadata.is_file()
    }
}

impl StreamRotateFS {
    /// Create a new StreamRotateFS with default configuration.
    pub fn new() -> Self {
        Self::with_config(RotateConfig::default())
    }

    /// Create a new StreamRotateFS with custom configuration.
    pub fn with_config(config: RotateConfig) -> Self {
        let config = Arc::new(config);
        let state = RotateState {
            current: FileEntry::file(PathBuf::from("/current"), Vec::new()),
            archive: BTreeMap::new(),
            next_seq: 1,
        };

        Self {
            config,
            state: Arc::new(RwLock::new(state)),
        }
    }

    pub(super) fn config_string(&self) -> String {
        format!(
            "max_size={}\nmax_files={}\ncompress={}\nbase_path={}\n",
            self.config.max_size,
            self.config.max_files,
            self.config.compress,
            self.config.base_path.display()
        )
    }

    pub(super) fn rotate(&self) -> Result<(), FSError> {
        let mut state = self.state.write();

        let current_data = std::mem::take(&mut state.current.data);
        let current_path = PathBuf::from("/archive/001.log");

        let archive_entry = FileEntry::file(current_path.clone(), current_data);
        let rotated_seq = state.next_seq;
        state.archive.insert(rotated_seq, archive_entry);

        state.next_seq += 1;

        while state.archive.len() > self.config.max_files {
            let oldest_seq = state.archive.first_key_value().map(|(k, _)| *k);
            if let Some(seq) = oldest_seq {
                tracing::debug!(seq = seq, "pruning old archive file");
                state.archive.remove(&seq);
            } else {
                break;
            }
        }

        state.current = FileEntry::file(PathBuf::from("/current"), Vec::new());
        state.current.metadata.modified = Some(Utc::now());

        tracing::debug!(
            seq = rotated_seq,
            size = state
                .archive
                .get(&rotated_seq)
                .map(|e| e.data.len())
                .unwrap_or(0),
            "rotated log file"
        );

        Ok(())
    }

    pub(super) fn check_rotate(&self) -> Result<(), FSError> {
        let state = self.state.read();
        let needs_rotation = state.current.data.len() >= self.config.max_size;
        drop(state);

        if needs_rotation {
            self.rotate()?;
        }

        Ok(())
    }

    pub(super) fn normalize_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        if path_str == "/" {
            return PathBuf::from("/");
        }

        let normalized = path_str.trim_start_matches('/');
        if normalized.is_empty() {
            PathBuf::from("/")
        } else {
            PathBuf::from("/").join(normalized)
        }
    }

    pub(super) fn validate_path(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        let path_str = normalized.to_string_lossy();

        let valid = path_str == "/"
            || path_str == "/current"
            || path_str == "/rotate"
            || path_str == "/config"
            || path_str == "/archive"
            || path_str.starts_with("/archive/");

        if !valid {
            return Err(FSError::NotFound { path: normalized });
        }

        Ok(())
    }

    pub(super) fn get_entry(&self, path: &Path) -> Result<FileEntry, FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();
        let state = self.state.read();

        match path_str.as_str() {
            "/" => Ok(FileEntry::dir(PathBuf::from("/"))),
            "/current" => Ok(state.current.clone()),
            "/rotate" => Ok(FileEntry::file(PathBuf::from("/rotate"), Vec::new())),
            "/config" => Ok(FileEntry::file(
                PathBuf::from("/config"),
                self.config_string().into_bytes(),
            )),
            "/archive" => Ok(FileEntry::dir(PathBuf::from("/archive"))),
            path_str_ref if path_str_ref.starts_with("/archive/") => {
                let name = path_str_ref.strip_prefix("/archive/").unwrap_or("");
                let seq_str = name.strip_suffix(".log").unwrap_or(name);

                if let Ok(seq) = seq_str.parse::<usize>()
                    && let Some(entry) = state.archive.get(&seq)
                {
                    return Ok(entry.clone());
                }
                Err(FSError::NotFound { path: normalized })
            }
            _ => Err(FSError::NotFound { path: normalized }),
        }
    }
}

impl Default for StreamRotateFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for StreamRotateFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        if path_str == "/current" {
            let mut state = self.state.write();
            state.current.data.truncate(size as usize);
            state.current.metadata.size = size;
            state.current.metadata.modified = Some(Utc::now());
            Ok(())
        } else {
            Err(FSError::NotSupported {
                message: "truncate only supported for /current".to_string(),
            })
        }
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.validate_path(&normalized)?;

        let path_str: String = normalized.to_string_lossy().to_string();

        match path_str.as_str() {
            "/current" => {
                let mut state = self.state.write();
                state.current.metadata.accessed = Some(Utc::now());
                state.current.metadata.modified = Some(Utc::now());
                Ok(())
            }
            "/rotate" | "/config" => Ok(()),
            "/" | "/archive" => Ok(()),
            path_str_ref if path_str_ref.starts_with("/archive/") => {
                let name = path_str_ref.strip_prefix("/archive/").unwrap_or("");
                let seq_str = name.strip_suffix(".log").unwrap_or(name);

                if let Ok(seq) = seq_str.parse::<usize>() {
                    let state = self.state.read();
                    if state.archive.contains_key(&seq) {
                        return Ok(());
                    }
                }
                Err(FSError::NotFound { path: normalized })
            }
            _ => Err(FSError::NotFound { path: normalized }),
        }
    }
}
