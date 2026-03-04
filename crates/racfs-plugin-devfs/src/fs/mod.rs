//! Device filesystem (devfs) implementation.

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
use racfs_core::{
    error::FSError,
    metadata::{FileMetadata, S_IFCHR},
};

/// Virtual device types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// /dev/null - discards all writes, returns EOF on read
    Null,
    /// /dev/zero - returns zeros on read
    Zero,
    /// /dev/random - returns random bytes (blocking)
    Random,
    /// /dev/urandom - returns random bytes (non-blocking)
    URandom,
}

/// Device filesystem providing virtual device files.
pub struct DevFS {
    pub(super) devices: Arc<RwLock<HashMap<PathBuf, DeviceType>>>,
}

impl DevFS {
    /// Create a new device filesystem.
    pub fn new() -> Self {
        let fs = Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
        };

        // Register default devices
        fs.register_device(PathBuf::from("/dev/null"), DeviceType::Null)
            .ok();
        fs.register_device(PathBuf::from("/dev/zero"), DeviceType::Zero)
            .ok();
        fs.register_device(PathBuf::from("/dev/random"), DeviceType::Random)
            .ok();
        fs.register_device(PathBuf::from("/dev/urandom"), DeviceType::URandom)
            .ok();

        fs
    }

    /// Register a device.
    pub fn register_device(&self, path: PathBuf, device_type: DeviceType) -> Result<(), FSError> {
        let mut devices = self.devices.write();
        devices.insert(path, device_type);
        Ok(())
    }

    pub(super) fn get_device(&self, path: &Path) -> Option<DeviceType> {
        let devices = self.devices.read();
        devices.get(path).copied()
    }

    pub(super) fn create_device_metadata(path: &Path, _device_type: DeviceType) -> FileMetadata {
        let mut metadata = FileMetadata::new(path.to_path_buf(), S_IFCHR | 0o666);
        metadata.size = 0;
        metadata
    }
}

impl Default for DevFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl racfs_core::FileSystem for DevFS {}
