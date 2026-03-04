//! StreamFS filesystem implementation.

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
use parking_lot::RwLock;
use racfs_core::filesystem::ReadFS;
use racfs_core::{error::FSError, filesystem::FileSystem};
use serde::{Deserialize, Serialize};

use racfs_core::Compression;

/// Stored message payload: raw bytes or compressed (when compression is enabled in config).
#[derive(Debug, Clone)]
pub(super) enum MessageStorage {
    Raw(Vec<u8>),
    Compressed {
        decompressed_len: u32,
        data: Vec<u8>,
    },
}

impl MessageStorage {
    pub(super) fn len(&self) -> usize {
        match self {
            MessageStorage::Raw(d) => d.len(),
            MessageStorage::Compressed {
                decompressed_len, ..
            } => *decompressed_len as usize,
        }
    }
}

/// Configuration for StreamFS.
#[derive(Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Ring buffer size (max messages per stream)
    pub buffer_size: usize,
    /// Max history entries
    pub history_size: usize,
    /// Max concurrent streams
    pub max_streams: usize,
    /// Optional compressor for message bodies. Set to `Some(...)` in config to enable; `None` to disable.
    #[serde(skip)]
    pub compression: Option<Arc<dyn Compression + Send + Sync>>,
}

impl std::fmt::Debug for StreamConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("StreamConfig");
        s.field("buffer_size", &self.buffer_size)
            .field("history_size", &self.history_size)
            .field("max_streams", &self.max_streams)
            .field("compression", &self.compression.is_some());
        s.finish()
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            history_size: 100,
            max_streams: 100,
            compression: None,
        }
    }
}

/// Streaming data filesystem.
///
/// Provides a virtual filesystem for streaming data:
/// - `/streams/{name}/` - Stream directories
/// - `/streams/{name}/data/` - Message files (000001.msg, 000002.msg, etc.)
/// - `/streams/{name}/head` - Next message ID to read
/// - `/streams/{name}/tail` - Next message ID to write
/// - `/streams/{name}/config` - Stream config (read-only)
pub struct StreamFS {
    pub(super) config: Arc<StreamConfig>,
    pub(super) state: Arc<RwLock<StreamState>>,
}

#[derive(Debug, Default)]
pub(super) struct StreamState {
    pub(super) streams: BTreeMap<String, Stream>,
}

#[derive(Debug, Clone)]
pub(super) struct Stream {
    pub(super) name: String,
    pub(super) messages: BTreeMap<u64, MessageStorage>,
    pub(super) head: u64,
    pub(super) tail: u64,
}

impl Stream {
    pub(super) fn new(name: String) -> Self {
        Self {
            name,
            messages: BTreeMap::new(),
            head: 1,
            tail: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum PathComponent {
    Root,
    StreamsDir,
    StreamDir(String),
    DataDir(String),
    Message(String, u64),
    Head(String),
    Tail(String),
    Config(String),
}

impl StreamFS {
    /// Create a new streaming filesystem.
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config: Arc::new(config),
            state: Arc::new(RwLock::new(StreamState::default())),
        }
    }

    /// Create with default configuration.
    pub fn default_config() -> Self {
        Self::new(StreamConfig::default())
    }

    pub(super) fn normalize_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        if path_str.starts_with('/') {
            path.to_path_buf()
        } else {
            PathBuf::from("/").join(path)
        }
    }

    pub(super) fn parse_path(&self, path: &Path) -> Option<PathComponent> {
        let parts: Vec<&str> = path
            .to_str()?
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        match parts.as_slice() {
            [] => Some(PathComponent::Root),
            ["streams"] => Some(PathComponent::StreamsDir),
            ["streams", stream_name] => Some(PathComponent::StreamDir(stream_name.to_string())),
            ["streams", stream_name, "data"] => {
                Some(PathComponent::DataDir(stream_name.to_string()))
            }
            ["streams", stream_name, "data", filename] if filename.ends_with(".msg") => {
                let id_str = filename.trim_end_matches(".msg");
                let id: u64 = id_str.parse().ok()?;
                Some(PathComponent::Message(stream_name.to_string(), id))
            }
            ["streams", stream_name, "head"] => Some(PathComponent::Head(stream_name.to_string())),
            ["streams", stream_name, "tail"] => Some(PathComponent::Tail(stream_name.to_string())),
            ["streams", stream_name, "config"] => {
                Some(PathComponent::Config(stream_name.to_string()))
            }
            _ => None,
        }
    }

    pub(super) fn create_stream(&self, name: &str) -> Result<(), FSError> {
        let mut state = self.state.write();
        if state.streams.len() >= self.config.max_streams {
            return Err(FSError::StorageFull);
        }
        if state.streams.contains_key(name) {
            return Err(FSError::AlreadyExists {
                path: PathBuf::from(format!("/streams/{}", name)),
            });
        }
        state
            .streams
            .insert(name.to_string(), Stream::new(name.to_string()));
        Ok(())
    }

    /// Convert stored message payload to bytes (decompress if needed).
    pub(super) fn message_storage_to_bytes(
        &self,
        stored: &MessageStorage,
    ) -> Result<Vec<u8>, FSError> {
        match stored {
            MessageStorage::Raw(d) => Ok(d.clone()),
            MessageStorage::Compressed { data, .. } => self
                .config
                .compression
                .as_ref()
                .ok_or_else(|| FSError::InvalidInput {
                    message: "compression not configured".to_string(),
                })?
                .decompress(data),
        }
    }

    /// Store bytes as message payload (compress if configured).
    pub(super) fn bytes_to_message_storage(&self, data: &[u8]) -> Result<MessageStorage, FSError> {
        if let Some(ref c) = self.config.compression {
            let compressed = c.compress(data)?;
            let len = data.len();
            let decompressed_len = u32::try_from(len).map_err(|_| FSError::InvalidInput {
                message: "message too large for compression format".to_string(),
            })?;
            return Ok(MessageStorage::Compressed {
                decompressed_len,
                data: compressed,
            });
        }
        Ok(MessageStorage::Raw(data.to_vec()))
    }
}

#[async_trait]
impl FileSystem for StreamFS {
    async fn truncate(&self, path: &Path, _size: u64) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        Err(FSError::PermissionDenied { path: normalized })
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let normalized = self.normalize_path(path);
        self.stat(&normalized).await?;
        Ok(())
    }
}
