//! Flags for file open and write operations.

use std::fmt;

bitflags::bitflags! {
    /// Flags for opening files.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct OpenFlags: u32 {
        /// Open file for reading.
        const READ = 0x0001;
        /// Open file for writing.
        const WRITE = 0x0002;
        /// Append to file (seek to end before each write).
        const APPEND = 0x0004;
        /// Create file if it doesn't exist.
        const CREATE = 0x0008;
        /// Fail if file exists (used with CREATE).
        const EXCLUSIVE = 0x0010;
        /// Truncate file to zero length.
        const TRUNCATE = 0x0020;
        /// Synchronous I/O (write returns after data is on disk).
        const SYNC = 0x0040;
        /// Don't update file access time.
        const NO_ATIME = 0x0080;
        /// Non-blocking mode.
        const NON_BLOCK = 0x0100;
        /// Directory mode.
        const DIRECTORY = 0x0200;
    }
}

impl OpenFlags {
    pub fn read() -> Self {
        Self::READ
    }

    pub fn write() -> Self {
        Self::READ | Self::WRITE
    }

    pub fn read_write() -> Self {
        Self::READ | Self::WRITE
    }

    pub fn create() -> Self {
        Self::READ | Self::WRITE | Self::CREATE
    }

    pub fn create_truncate() -> Self {
        Self::READ | Self::WRITE | Self::CREATE | Self::TRUNCATE
    }

    pub fn is_read_only(&self) -> bool {
        self.bits() & Self::WRITE.bits() == 0
    }

    pub fn is_write_only(&self) -> bool {
        (self.bits() & Self::READ.bits()) == 0 && (self.bits() & Self::WRITE.bits()) != 0
    }

    pub fn is_read_write(&self) -> bool {
        (self.bits() & Self::READ.bits()) != 0 && (self.bits() & Self::WRITE.bits()) != 0
    }

    pub fn contains_create(&self) -> bool {
        self.contains(Self::CREATE)
    }

    pub fn contains_truncate(&self) -> bool {
        self.contains(Self::TRUNCATE)
    }

    pub fn contains_append(&self) -> bool {
        self.contains(Self::APPEND)
    }

    pub fn contains_sync(&self) -> bool {
        self.contains(Self::SYNC)
    }

    pub fn contains_directory(&self) -> bool {
        self.contains(Self::DIRECTORY)
    }
}

impl fmt::Display for OpenFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.contains(Self::READ) {
            flags.push("READ");
        }
        if self.contains(Self::WRITE) {
            flags.push("WRITE");
        }
        if self.contains(Self::APPEND) {
            flags.push("APPEND");
        }
        if self.contains(Self::CREATE) {
            flags.push("CREATE");
        }
        if self.contains(Self::EXCLUSIVE) {
            flags.push("EXCLUSIVE");
        }
        if self.contains(Self::TRUNCATE) {
            flags.push("TRUNCATE");
        }
        if self.contains(Self::SYNC) {
            flags.push("SYNC");
        }
        if self.contains(Self::NO_ATIME) {
            flags.push("NO_ATIME");
        }
        if self.contains(Self::NON_BLOCK) {
            flags.push("NON_BLOCK");
        }
        if self.contains(Self::DIRECTORY) {
            flags.push("DIRECTORY");
        }
        write!(f, "{}", flags.join("|"))
    }
}

bitflags::bitflags! {
    /// Flags for write operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct WriteFlags: u32 {
        /// No special flags.
        const NONE = 0x0000;
        /// Synchronous write (return after data is written to disk).
        const SYNC = 0x0001;
        /// Append to end of file.
        const APPEND = 0x0002;
    }
}

impl WriteFlags {
    pub fn none() -> Self {
        Self::NONE
    }

    pub fn sync() -> Self {
        Self::SYNC
    }

    pub fn append() -> Self {
        Self::APPEND
    }

    pub fn contains_sync(&self) -> bool {
        self.contains(Self::SYNC)
    }

    pub fn contains_append(&self) -> bool {
        self.contains(Self::APPEND)
    }
}

impl fmt::Display for WriteFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "NONE")
        } else {
            let mut flags = Vec::new();
            if self.contains(Self::SYNC) {
                flags.push("SYNC");
            }
            if self.contains(Self::APPEND) {
                flags.push("APPEND");
            }
            write!(f, "{}", flags.join("|"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_flags_helpers() {
        assert!(OpenFlags::read().contains(OpenFlags::READ));
        assert!(OpenFlags::write().contains(OpenFlags::WRITE));
        assert!(OpenFlags::read_write().contains(OpenFlags::READ));
        assert!(OpenFlags::read_write().contains(OpenFlags::WRITE));
        assert!(OpenFlags::create().contains(OpenFlags::CREATE));
        assert!(OpenFlags::create_truncate().contains(OpenFlags::TRUNCATE));
        assert!(OpenFlags::READ.is_read_only());
        assert!(!OpenFlags::WRITE.is_read_only());
        assert!(OpenFlags::WRITE.is_write_only());
        assert!((OpenFlags::READ | OpenFlags::WRITE).is_read_write());
        assert!(OpenFlags::create().contains_create());
        assert!(OpenFlags::create_truncate().contains_truncate());
        assert!((OpenFlags::APPEND).contains_append());
        assert!((OpenFlags::SYNC).contains_sync());
        assert!((OpenFlags::DIRECTORY).contains_directory());
    }

    #[test]
    fn open_flags_display() {
        assert_eq!(OpenFlags::READ.to_string(), "READ");
        assert_eq!(
            (OpenFlags::READ | OpenFlags::WRITE).to_string(),
            "READ|WRITE"
        );
        assert!(
            (OpenFlags::READ | OpenFlags::APPEND)
                .to_string()
                .contains("APPEND")
        );
    }

    #[test]
    fn write_flags_helpers() {
        assert!(WriteFlags::none().is_empty());
        assert!(WriteFlags::sync().contains_sync());
        assert!(WriteFlags::append().contains_append());
    }

    #[test]
    fn write_flags_display() {
        assert_eq!(WriteFlags::NONE.to_string(), "NONE");
        assert_eq!(WriteFlags::SYNC.to_string(), "SYNC");
        assert_eq!(WriteFlags::APPEND.to_string(), "APPEND");
        assert!(
            (WriteFlags::SYNC | WriteFlags::APPEND)
                .to_string()
                .contains("SYNC")
        );
    }
}
