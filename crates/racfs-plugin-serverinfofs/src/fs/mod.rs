//! ServerInfoFS filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use racfs_core::{error::FSError, filesystem::FileSystem, metadata::FileMetadata};

/// Server information filesystem.
///
/// Provides a read-only filesystem with server status information:
/// - `/version` - Server version
/// - `/uptime` - Server uptime in seconds
/// - `/hostname` - System hostname
/// - `/memory/total` - Total memory (bytes)
/// - `/memory/used` - Used memory (bytes)
/// - `/memory/available` - Available memory (bytes)
/// - `/cpu/count` - CPU core count
/// - `/cpu/load_avg` - Load average
/// - `/plugins/list` - Loaded plugins
pub struct ServerInfoFS {
    /// File entries stored in memory
    entries: Arc<RwLock<Vec<ServerInfoEntry>>>,
    /// Server start time for uptime calculation
    start_time: Instant,
}

/// A file entry in the serverinfo filesystem.
#[derive(Clone)]
pub(super) struct ServerInfoEntry {
    /// File path
    pub(super) path: PathBuf,
    /// File content
    pub(super) content: Vec<u8>,
    /// File metadata
    pub(super) metadata: FileMetadata,
}

impl ServerInfoEntry {
    /// Create a new file entry.
    pub(super) fn file(path: PathBuf, content: Vec<u8>) -> Self {
        let size = content.len() as u64;
        Self {
            path: path.clone(),
            content,
            metadata: FileMetadata::file(path, size),
        }
    }
}

impl ServerInfoFS {
    /// Create a new server information filesystem.
    pub fn new() -> Self {
        let fs = Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
        };
        fs.initialize_entries();
        fs
    }

    /// Initialize all server information entries.
    fn initialize_entries(&self) {
        let mut entries = self.entries.write();

        entries.push(ServerInfoEntry {
            path: PathBuf::from("/"),
            content: Vec::new(),
            metadata: FileMetadata::directory(PathBuf::from("/")),
        });

        entries.push(ServerInfoEntry::file(
            PathBuf::from("/version"),
            "0.1.0".as_bytes().to_vec(),
        ));
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/uptime"),
            self.get_uptime().as_bytes().to_vec(),
        ));
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/hostname"),
            self.get_hostname().as_bytes().to_vec(),
        ));

        entries.push(ServerInfoEntry {
            path: PathBuf::from("/memory"),
            content: Vec::new(),
            metadata: FileMetadata::directory(PathBuf::from("/memory")),
        });
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/memory/total"),
            "8589934592".as_bytes().to_vec(),
        ));
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/memory/used"),
            "4294967296".as_bytes().to_vec(),
        ));
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/memory/available"),
            "4294967296".as_bytes().to_vec(),
        ));

        entries.push(ServerInfoEntry {
            path: PathBuf::from("/cpu"),
            content: Vec::new(),
            metadata: FileMetadata::directory(PathBuf::from("/cpu")),
        });
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/cpu/count"),
            num_cpus::get().to_string().as_bytes().to_vec(),
        ));
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/cpu/load_avg"),
            "0.50 0.40 0.30".as_bytes().to_vec(),
        ));

        entries.push(ServerInfoEntry {
            path: PathBuf::from("/plugins"),
            content: Vec::new(),
            metadata: FileMetadata::directory(PathBuf::from("/plugins")),
        });
        entries.push(ServerInfoEntry::file(
            PathBuf::from("/plugins/list"),
            "hellofs,heartbeatfs,serverinfofs".as_bytes().to_vec(),
        ));
    }

    /// Get current uptime in seconds.
    pub(super) fn get_uptime(&self) -> String {
        self.start_time.elapsed().as_secs().to_string()
    }

    /// Get system hostname.
    pub(super) fn get_hostname(&self) -> String {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Update dynamic values (uptime).
    pub fn update(&self) {
        let mut entries = self.entries.write();
        if let Some(entry) = entries.iter_mut().find(|e| e.path == Path::new("/uptime")) {
            entry.content = self.get_uptime().as_bytes().to_vec();
            entry.metadata.size = entry.content.len() as u64;
            entry.metadata.modified = Some(Utc::now());
        }
    }

    /// Get an entry by path.
    pub(super) fn get_entry(&self, path: &Path) -> Result<ServerInfoEntry, FSError> {
        let entries = self.entries.read();
        entries
            .iter()
            .find(|e| e.path == path)
            .cloned()
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })
    }

    /// Expose entries for read_dir (needs to read the list).
    pub(super) fn entries(&self) -> parking_lot::RwLockReadGuard<'_, Vec<ServerInfoEntry>> {
        self.entries.read()
    }
}

impl Default for ServerInfoFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for ServerInfoFS {
    async fn truncate(&self, path: &Path, _size: u64) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        Err(FSError::PermissionDenied {
            path: path.to_path_buf(),
        })
    }
}
