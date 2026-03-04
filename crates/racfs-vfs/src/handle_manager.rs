//! Handle manager for tracking open file handles.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::RwLock;
use racfs_core::{
    FSError,
    file_handle::{FileHandle, HandleId},
    flags::OpenFlags,
};

/// Default lease timeout in seconds.
const DEFAULT_LEASE_TIMEOUT_SECS: u64 = 30;

/// Maximum number of open handles.
const MAX_HANDLES: usize = 1024;

/// Composite key for handle lookup: (path, flags)
type HandleKey = (PathBuf, OpenFlags);

/// Manages file handles with lease mechanism.
///
/// Tracks open file handles and automatically expires them after a timeout.
/// Uses dual indexing: by HandleId for direct access, by (path, flags) for deduplication.
pub struct HandleManager {
    handles: Arc<RwLock<HashMap<HandleId, HandleEntry>>>,
    /// Secondary index: path+flags -> handle_id for fast duplicate detection
    path_index: Arc<RwLock<HashMap<HandleKey, HandleId>>>,
    lease_timeout: Duration,
    max_handles: usize,
}

#[allow(dead_code)]
struct HandleEntry {
    handle: FileHandle,
    created: Instant,
    last_accessed: Instant,
}

impl HandleManager {
    /// Create a new handle manager.
    pub fn new() -> Self {
        Self {
            handles: Arc::new(RwLock::new(HashMap::new())),
            path_index: Arc::new(RwLock::new(HashMap::new())),
            lease_timeout: Duration::from_secs(DEFAULT_LEASE_TIMEOUT_SECS),
            max_handles: MAX_HANDLES,
        }
    }

    /// Create a handle manager with custom settings.
    pub fn with_config(lease_timeout_secs: u64, max_handles: usize) -> Self {
        Self {
            handles: Arc::new(RwLock::new(HashMap::new())),
            path_index: Arc::new(RwLock::new(HashMap::new())),
            lease_timeout: Duration::from_secs(lease_timeout_secs),
            max_handles,
        }
    }

    /// Open a new file handle.
    pub fn open_handle(&self, path: PathBuf, flags: OpenFlags) -> Result<FileHandle, FSError> {
        // Check max handles limit first (read-only check, no lock needed for this check)
        if self.handles.read().len() >= self.max_handles {
            return Err(FSError::TooManyOpenFiles);
        }

        // Check if handle for same path with same flags already exists (O(1) with index)
        let key = (path.clone(), flags);
        if let Some(existing_id) = self.path_index.read().get(&key).copied() {
            let handles = self.handles.read();
            if let Some(entry) = handles.get(&existing_id) {
                return Ok(entry.handle.clone());
            }
        }

        let mut handles = self.handles.write();
        let mut path_index = self.path_index.write();

        // Double-check after acquiring write lock
        if let Some(existing_id) = path_index.get(&key).copied()
            && let Some(entry) = handles.get(&existing_id)
        {
            return Ok(entry.handle.clone());
        }

        let handle = FileHandle::new(path, flags);
        let id = handle.id;

        handles.insert(
            id,
            HandleEntry {
                handle: handle.clone(),
                created: Instant::now(),
                last_accessed: Instant::now(),
            },
        );

        path_index.insert(key, id);

        tracing::debug!(handle_id = %id, "opened new file handle");

        Ok(handle)
    }

    /// Close a file handle.
    pub fn close_handle(&self, handle_id: &HandleId) -> Result<(), FSError> {
        let mut handles = self.handles.write();
        let mut path_index = self.path_index.write();

        // Find and remove the handle entry to get the path key
        let key = if let Some(entry) = handles.get(handle_id) {
            (entry.handle.path.clone(), entry.handle.flags)
        } else {
            tracing::warn!(handle_id = %handle_id, "attempted to close non-existent handle");
            return Err(FSError::InvalidHandle {
                handle_id: handle_id.to_string(),
            });
        };

        handles.remove(handle_id);
        path_index.remove(&key);

        tracing::debug!(handle_id = %handle_id, "closed file handle");
        Ok(())
    }

    /// Get a handle by ID.
    pub fn get_handle(&self, handle_id: &HandleId) -> Result<FileHandle, FSError> {
        let mut handles = self.handles.write();

        let entry = handles
            .get_mut(handle_id)
            .ok_or_else(|| FSError::InvalidHandle {
                handle_id: handle_id.to_string(),
            })?;

        // Update last accessed time
        entry.last_accessed = Instant::now();

        Ok(entry.handle.clone())
    }

    /// Renew the lease on a handle.
    pub fn renew_lease(&self, handle_id: &HandleId) -> Result<(), FSError> {
        let mut handles = self.handles.write();

        let entry = handles
            .get_mut(handle_id)
            .ok_or_else(|| FSError::InvalidHandle {
                handle_id: handle_id.to_string(),
            })?;

        entry.last_accessed = Instant::now();
        Ok(())
    }

    /// Update the offset of a handle.
    pub fn update_offset(&self, handle_id: &HandleId, offset: i64) -> Result<(), FSError> {
        let mut handles = self.handles.write();

        let entry = handles
            .get_mut(handle_id)
            .ok_or_else(|| FSError::InvalidHandle {
                handle_id: handle_id.to_string(),
            })?;

        entry.handle.offset = offset;
        entry.last_accessed = Instant::now();
        Ok(())
    }

    /// Clean up expired handles.
    ///
    /// Returns the number of handles removed.
    pub fn cleanup_expired(&self) -> usize {
        let now = Instant::now();
        let mut handles = self.handles.write();
        let mut path_index = self.path_index.write();
        let _before = handles.len();

        // Collect expired handle IDs first
        let expired: Vec<HandleId> = handles
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_accessed) >= self.lease_timeout)
            .map(|(id, entry)| {
                // Also remove from path index
                let key = (entry.handle.path.clone(), entry.handle.flags);
                path_index.remove(&key);
                *id
            })
            .collect();

        // Remove expired handles
        for id in &expired {
            handles.remove(id);
        }

        let removed = expired.len();
        if removed > 0 {
            tracing::info!(removed = removed, "cleaned up expired handles");
        }

        removed
    }

    /// Get the number of open handles.
    pub fn len(&self) -> usize {
        self.handles.read().len()
    }

    /// Check if there are any open handles.
    pub fn is_empty(&self) -> bool {
        self.handles.read().is_empty()
    }

    /// List all open handles.
    pub fn list_handles(&self) -> Vec<HandleId> {
        self.handles.read().keys().copied().collect()
    }
}

impl Default for HandleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_close_handle() {
        let manager = HandleManager::new();
        let path = PathBuf::from("/test/file.txt");
        let flags = OpenFlags::read();

        let handle = manager.open_handle(path.clone(), flags).unwrap();
        assert!(manager.get_handle(&handle.id).is_ok());

        manager.close_handle(&handle.id).unwrap();
        assert!(manager.get_handle(&handle.id).is_err());
    }

    #[test]
    fn test_duplicate_handles() {
        let manager = HandleManager::new();
        let path = PathBuf::from("/test/file.txt");
        let flags = OpenFlags::read();

        let handle1 = manager.open_handle(path.clone(), flags).unwrap();
        let handle2 = manager.open_handle(path, flags).unwrap();

        // Same path and flags should return same handle
        assert_eq!(handle1.id, handle2.id);
    }

    #[test]
    fn test_cleanup_expired() {
        let manager = HandleManager::with_config(0, 100);
        let path = PathBuf::from("/test/file.txt");
        let flags = OpenFlags::read();

        let _handle = manager.open_handle(path, flags).unwrap();
        assert_eq!(manager.len(), 1);

        // Wait for lease to expire
        std::thread::sleep(std::time::Duration::from_millis(10));

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_list_handles() {
        let manager = HandleManager::new();
        assert!(manager.list_handles().is_empty());
        let h1 = manager
            .open_handle(PathBuf::from("/a"), OpenFlags::read())
            .unwrap();
        let h2 = manager
            .open_handle(PathBuf::from("/b"), OpenFlags::read())
            .unwrap();
        let ids = manager.list_handles();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&h1.id));
        assert!(ids.contains(&h2.id));
    }

    #[test]
    fn test_get_handle_invalid_returns_error() {
        let manager = HandleManager::new();
        let path = PathBuf::from("/x");
        let handle = manager.open_handle(path, OpenFlags::read()).unwrap();
        manager.close_handle(&handle.id).unwrap();
        let err = manager.get_handle(&handle.id).unwrap_err();
        assert!(matches!(err, FSError::InvalidHandle { .. }));
    }

    #[test]
    fn test_is_empty_and_len() {
        let manager = HandleManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        let _ = manager
            .open_handle(PathBuf::from("/f"), OpenFlags::read())
            .unwrap();
        assert!(!manager.is_empty());
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_default_equals_new() {
        let default_manager = HandleManager::default();
        let new_manager = HandleManager::new();
        assert_eq!(default_manager.len(), new_manager.len());
        assert!(default_manager.is_empty());
    }

    #[test]
    fn test_max_handles_rejects_new_handle() {
        let manager = HandleManager::with_config(3600, 0);
        let err = manager
            .open_handle(PathBuf::from("/any"), OpenFlags::read())
            .unwrap_err();
        assert!(matches!(err, FSError::TooManyOpenFiles));
    }

    #[test]
    fn test_renew_lease_and_update_offset() {
        let manager = HandleManager::new();
        let handle = manager
            .open_handle(PathBuf::from("/f"), OpenFlags::read())
            .unwrap();
        manager.renew_lease(&handle.id).unwrap();
        manager.update_offset(&handle.id, 100).unwrap();
        let h = manager.get_handle(&handle.id).unwrap();
        assert_eq!(h.offset, 100);
    }
}
