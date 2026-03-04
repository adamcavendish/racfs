//! HeartbeatFS filesystem implementation.

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
use racfs_core::{error::FSError, filesystem::FileSystem};

/// Health monitoring filesystem with heartbeat tracking.
///
/// Provides a simple filesystem interface for monitoring health status:
/// - `/status`: Current status ("ok" or "error")
/// - `/uptime`: Seconds since filesystem start
/// - `/beats`: Number of heartbeats recorded
/// - `/last_beat`: Timestamp of last heartbeat
/// - `/pulse`: Write "beat" to trigger a heartbeat
pub struct HeartbeatFS {
    /// Inner state protected by RwLock for thread-safe access.
    pub(crate) inner: Arc<RwLock<HeartbeatState>>,
}

/// Internal state of the heartbeat filesystem.
pub(super) struct HeartbeatState {
    /// Time when the filesystem was created.
    start_time: Instant,
    /// Number of heartbeats received.
    pub(super) beats: u64,
    /// Timestamp of the last heartbeat.
    last_beat: chrono::DateTime<Utc>,
    /// Current health status.
    status: HealthStatus,
    /// File data stored in memory (POSIX compliant).
    files: Vec<(PathBuf, Vec<u8>)>,
}

/// Health status values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum HealthStatus {
    Ok,
    #[allow(dead_code)]
    Error,
}

impl HeartbeatState {
    /// Create a new heartbeat state.
    pub(super) fn new() -> Self {
        let now = Utc::now();
        Self {
            start_time: Instant::now(),
            beats: 0,
            last_beat: now,
            status: HealthStatus::Ok,
            files: Vec::new(),
        }
    }

    /// Update file data from current state.
    pub(super) fn refresh_files(&mut self) {
        self.files.clear();

        let uptime = self.start_time.elapsed().as_secs();
        let beats = self.beats;
        let last_beat = self.last_beat.to_rfc3339();
        let status = match self.status {
            HealthStatus::Ok => "ok",
            HealthStatus::Error => "error",
        };

        self.files
            .push((PathBuf::from("/status"), status.as_bytes().to_vec()));
        self.files.push((
            PathBuf::from("/uptime"),
            uptime.to_string().as_bytes().to_vec(),
        ));
        self.files.push((
            PathBuf::from("/beats"),
            beats.to_string().as_bytes().to_vec(),
        ));
        self.files
            .push((PathBuf::from("/last_beat"), last_beat.as_bytes().to_vec()));
    }

    /// Record a heartbeat.
    pub(super) fn heartbeat(&mut self) {
        self.beats += 1;
        self.last_beat = Utc::now();
        self.status = HealthStatus::Ok;
        self.refresh_files();
    }
}

impl HeartbeatFS {
    /// Create a new heartbeat filesystem.
    pub fn new() -> Self {
        let fs = Self {
            inner: Arc::new(RwLock::new(HeartbeatState::new())),
        };
        fs.inner.write().refresh_files();
        fs
    }

    /// Get a file entry by path.
    pub(super) fn get_entry(&self, path: &Path) -> Result<Vec<u8>, FSError> {
        let inner = self.inner.read();
        inner
            .files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, data)| data.clone())
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })
    }

    /// Check if a path is valid for this filesystem.
    #[allow(dead_code)]
    pub fn is_valid_path(&self, path: &Path) -> bool {
        matches!(
            path.to_str(),
            Some("/status")
                | Some("/uptime")
                | Some("/beats")
                | Some("/last_beat")
                | Some("/pulse")
        )
    }
}

impl Default for HeartbeatFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for HeartbeatFS {
    async fn truncate(&self, _path: &Path, _size: u64) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn touch(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}
