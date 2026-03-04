//! In-memory filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use parking_lot::RwLock;
use prometheus::{IntCounterVec, IntGauge, Registry};
use racfs_core::{error::FSError, filesystem::FileSystem, metadata::FileMetadata};

use racfs_vfs::PluginMetrics;

/// Extended attributes: path -> (name -> value)
type XattrMap = HashMap<PathBuf, HashMap<String, Vec<u8>>>;

/// In-memory filesystem using BTreeMap for storage.
pub struct MemFS {
    pub(super) files: Arc<RwLock<HashMap<PathBuf, FileEntry>>>,
    /// Extended attributes: path -> (name -> value)
    pub(super) xattrs: Arc<RwLock<XattrMap>>,
    /// Plugin-specific metrics (lazy-initialized when register() is called).
    metrics: RwLock<Option<MemFSMetrics>>,
}

/// Metrics for MemFS (registered when PluginMetrics::register is called).
pub struct MemFSMetrics {
    pub entries: IntGauge,
    pub operations_total: IntCounterVec,
}

#[derive(Clone)]
pub(super) struct FileEntry {
    pub(super) data: Vec<u8>,
    pub(super) metadata: FileMetadata,
    pub(super) is_symlink: bool,
    pub(super) symlink_target: Option<PathBuf>,
}

impl MemFS {
    /// Create a new in-memory filesystem.
    pub fn new() -> Self {
        let fs = Self {
            files: Arc::new(RwLock::new(HashMap::new())),
            xattrs: Arc::new(RwLock::new(HashMap::new())),
            metrics: RwLock::new(None),
        };

        // Create root directory (sync helper for initialization)
        fs.mkdir_sync(&PathBuf::from("/"), 0o755).ok();
        fs
    }

    /// Sync version of mkdir for internal use during initialization
    pub(super) fn mkdir_sync(&self, path: &Path, perm: u32) -> Result<(), FSError> {
        let mut files = self.files.write();

        if files.contains_key(path) {
            return Err(FSError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        let mut metadata = FileMetadata::directory(path.to_path_buf());
        metadata.set_permissions(perm);
        files.insert(
            path.to_path_buf(),
            FileEntry {
                data: Vec::new(),
                metadata,
                is_symlink: false,
                symlink_target: None,
            },
        );
        Ok(())
    }

    /// Create a new in-memory filesystem with pre-populated data.
    pub fn with_data(data: HashMap<PathBuf, Vec<u8>>) -> Self {
        let mut entries = HashMap::new();

        for (path, content) in data {
            let metadata = FileMetadata::file(path.clone(), content.len() as u64);
            entries.insert(
                path,
                FileEntry {
                    data: content,
                    metadata,
                    is_symlink: false,
                    symlink_target: None,
                },
            );
        }

        // Create root directory
        let root_metadata = FileMetadata::directory(PathBuf::from("/"));
        entries.insert(
            PathBuf::from("/"),
            FileEntry {
                data: Vec::new(),
                metadata: root_metadata,
                is_symlink: false,
                symlink_target: None,
            },
        );

        Self {
            files: Arc::new(RwLock::new(entries)),
            xattrs: Arc::new(RwLock::new(HashMap::new())),
            metrics: RwLock::new(None),
        }
    }

    pub(super) fn get_entry(&self, path: &Path) -> Result<FileEntry, FSError> {
        let files = self.files.read();
        files.get(path).cloned().ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })
    }

    #[allow(dead_code)]
    pub(super) fn get_entry_mut(&self, path: &Path) -> Result<FileEntry, FSError> {
        let files = self.files.read();
        files.get(path).cloned().ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })
    }

    pub(super) fn ensure_parent_exists(&self, path: &Path) -> Result<(), FSError> {
        if let Some(parent) = path.parent()
            && parent.as_os_str() != "/"
        {
            let files = self.files.read();
            let parent_entry = files.get(parent).ok_or_else(|| FSError::NotFound {
                path: parent.to_path_buf(),
            })?;

            if !parent_entry.metadata.is_directory() {
                return Err(FSError::NotADirectory {
                    path: parent.to_path_buf(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn inc_op(&self, operation: &str) {
        if let Some(ref m) = *self.metrics.read() {
            m.operations_total.with_label_values(&[operation]).inc();
        }
    }

    pub(super) fn inc_entries(&self, delta: i64) {
        if let Some(ref m) = *self.metrics.read() {
            m.entries.add(delta);
        }
    }
}

impl Default for MemFS {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginMetrics for MemFS {
    fn register(&self, registry: &Registry) -> Result<(), prometheus::Error> {
        let entries = IntGauge::new(
            "racfs_plugin_memfs_entries_total",
            "Total number of entries (files and directories) in the memfs instance",
        )?;
        let operations_total = IntCounterVec::new(
            prometheus::Opts::new(
                "racfs_plugin_memfs_operations_total",
                "Total number of operations by type on this memfs instance",
            ),
            &["operation"],
        )?;
        registry.register(Box::new(entries.clone()))?;
        registry.register(Box::new(operations_total.clone()))?;
        let count = self.files.read().len() as i64;
        entries.set(count);
        *self.metrics.write() = Some(MemFSMetrics {
            entries,
            operations_total,
        });
        Ok(())
    }
}

#[async_trait]
impl FileSystem for MemFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let mut files = self.files.write();

        let entry = files.get_mut(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let current_size = entry.data.len() as u64;
        if size < current_size {
            entry.data.truncate(size as usize);
        } else if size > current_size {
            entry.data.resize(size as usize, 0);
        }

        entry.metadata.size = size;
        entry.metadata.modified = Some(chrono::Utc::now());

        tracing::debug!(path = %path.display(), size = size, "truncated");
        self.inc_op("truncate");
        Ok(())
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let mut files = self.files.write();

        if let Some(entry) = files.get_mut(path) {
            entry.metadata.accessed = Some(chrono::Utc::now());
            entry.metadata.modified = Some(chrono::Utc::now());
        } else {
            // Create new file with current timestamps
            let metadata = FileMetadata::file(path.to_path_buf(), 0);
            files.insert(
                path.to_path_buf(),
                FileEntry {
                    data: Vec::new(),
                    metadata,
                    is_symlink: false,
                    symlink_target: None,
                },
            );
            self.inc_entries(1);
        }

        tracing::debug!(path = %path.display(), "touched");
        self.inc_op("touch");
        Ok(())
    }

    async fn readlink(&self, path: &Path) -> Result<PathBuf, FSError> {
        let entry = self.get_entry(path)?;
        if !entry.is_symlink {
            return Err(FSError::InvalidInput {
                message: "not a symlink".to_string(),
            });
        }
        entry
            .symlink_target
            .clone()
            .ok_or_else(|| FSError::InvalidInput {
                message: "symlink has no target".to_string(),
            })
    }

    async fn symlink(&self, target: &Path, link: &Path) -> Result<(), FSError> {
        self.ensure_parent_exists(link)?;

        let mut files = self.files.write();

        if files.contains_key(link) {
            return Err(FSError::AlreadyExists {
                path: link.to_path_buf(),
            });
        }

        let metadata = FileMetadata::symlink(link.to_path_buf(), target.to_path_buf());
        files.insert(
            link.to_path_buf(),
            FileEntry {
                data: Vec::new(),
                metadata,
                is_symlink: true,
                symlink_target: Some(target.to_path_buf()),
            },
        );

        tracing::debug!(target = %target.display(), link = %link.display(), "created symlink");
        self.inc_op("symlink");
        self.inc_entries(1);
        Ok(())
    }

    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Vec<u8>, FSError> {
        self.get_entry(path)?;
        let xattrs = self.xattrs.read();
        let per_path = xattrs.get(path).ok_or_else(|| FSError::InvalidInput {
            message: "extended attribute not found".to_string(),
        })?;
        per_path
            .get(name)
            .cloned()
            .ok_or_else(|| FSError::InvalidInput {
                message: "extended attribute not found".to_string(),
            })
    }

    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<(), FSError> {
        self.get_entry(path)?;
        self.xattrs
            .write()
            .entry(path.to_path_buf())
            .or_default()
            .insert(name.to_string(), value.to_vec());
        self.inc_op("set_xattr");
        Ok(())
    }

    async fn remove_xattr(&self, path: &Path, name: &str) -> Result<(), FSError> {
        self.get_entry(path)?;
        let mut xattrs = self.xattrs.write();
        let per_path = xattrs.get_mut(path).ok_or_else(|| FSError::InvalidInput {
            message: "extended attribute not found".to_string(),
        })?;
        per_path.remove(name).ok_or_else(|| FSError::InvalidInput {
            message: "extended attribute not found".to_string(),
        })?;
        self.inc_op("remove_xattr");
        Ok(())
    }

    async fn list_xattr(&self, path: &Path) -> Result<Vec<String>, FSError> {
        self.get_entry(path)?;
        let xattrs = self.xattrs.read();
        let names: Vec<String> = xattrs
            .get(path)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        self.inc_op("list_xattr");
        Ok(names)
    }
}
