//! File handle types.

use std::path::PathBuf;

use uuid::Uuid;

use crate::flags::OpenFlags;

/// Unique identifier for a file handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct HandleId(pub Uuid);

impl HandleId {
    /// Create a new unique handle ID.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Create a handle ID from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for HandleId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HandleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an open file handle.
#[derive(Debug, Clone)]
pub struct FileHandle {
    /// Unique handle identifier.
    pub id: HandleId,
    /// Path to the file.
    pub path: PathBuf,
    /// Flags used when opening.
    pub flags: OpenFlags,
    /// Current offset in the file.
    pub offset: i64,
    /// Whether the handle is still valid.
    pub valid: bool,
}

impl FileHandle {
    /// Create a new file handle.
    pub fn new(path: PathBuf, flags: OpenFlags) -> Self {
        Self {
            id: HandleId::new(),
            path,
            flags,
            offset: 0,
            valid: true,
        }
    }

    /// Create from an existing handle ID.
    pub fn with_id(path: PathBuf, flags: OpenFlags, id: HandleId) -> Self {
        Self {
            id,
            path,
            flags,
            offset: 0,
            valid: true,
        }
    }

    /// Check if the handle is open for reading.
    pub fn is_readable(&self) -> bool {
        self.flags.is_read_only() || self.flags.is_read_write()
    }

    /// Check if the handle is open for writing.
    pub fn is_writable(&self) -> bool {
        self.flags.is_write_only() || self.flags.is_read_write()
    }

    /// Advance the offset.
    pub fn advance(&mut self, count: i64) {
        self.offset = self.offset.saturating_add(count);
    }

    /// Seek to a specific offset.
    pub fn seek(&mut self, offset: i64) {
        self.offset = offset;
    }

    /// Get current offset.
    pub fn offset(&self) -> i64 {
        self.offset
    }

    /// Invalidate the handle.
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Check if the handle is valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_id_new_and_default() {
        let a = HandleId::new();
        let b = HandleId::default();
        assert_ne!(a.as_uuid(), b.as_uuid());
    }

    #[test]
    fn handle_id_from_uuid() {
        let u = Uuid::now_v7();
        let id = HandleId::from_uuid(u);
        assert_eq!(id.as_uuid(), u);
    }

    #[test]
    fn handle_id_display() {
        let id = HandleId::new();
        let s = id.to_string();
        assert!(!s.is_empty());
        assert!(s.len() >= 32);
    }

    #[test]
    fn file_handle_new() {
        let h = FileHandle::new(PathBuf::from("/foo"), OpenFlags::READ);
        assert_eq!(h.path, PathBuf::from("/foo"));
        assert_eq!(h.offset, 0);
        assert!(h.valid);
        assert!(h.is_readable());
        assert!(!h.is_writable());
    }

    #[test]
    fn file_handle_with_id() {
        let id = HandleId::new();
        let h = FileHandle::with_id(PathBuf::from("/bar"), OpenFlags::WRITE, id);
        assert_eq!(h.id.as_uuid(), id.as_uuid());
        assert!(h.is_writable());
    }

    #[test]
    fn file_handle_read_write_flags() {
        let r = FileHandle::new(PathBuf::from("/r"), OpenFlags::READ);
        assert!(r.is_readable());
        assert!(!r.is_writable());

        let w = FileHandle::new(PathBuf::from("/w"), OpenFlags::WRITE);
        assert!(!w.is_readable());
        assert!(w.is_writable());

        let rw = FileHandle::new(PathBuf::from("/rw"), OpenFlags::read_write());
        assert!(rw.is_readable());
        assert!(rw.is_writable());
    }

    #[test]
    fn file_handle_advance_and_seek() {
        let mut h = FileHandle::new(PathBuf::from("/f"), OpenFlags::READ);
        assert_eq!(h.offset(), 0);
        h.advance(10);
        assert_eq!(h.offset(), 10);
        h.advance(-3);
        assert_eq!(h.offset(), 7);
        h.seek(100);
        assert_eq!(h.offset(), 100);
    }

    #[test]
    fn file_handle_invalidate() {
        let mut h = FileHandle::new(PathBuf::from("/f"), OpenFlags::READ);
        assert!(h.is_valid());
        h.invalidate();
        assert!(!h.is_valid());
    }
}
