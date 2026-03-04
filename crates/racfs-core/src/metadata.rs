//! File metadata types.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// File type mask.
pub const S_IFMT: u32 = 0o170000;

/// File type constants.
pub const S_IFIFO: u32 = 0o010000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFLNK: u32 = 0o120000;
pub const S_IFSOCK: u32 = 0o140000;

/// Permission constants.
pub const S_IRWXU: u32 = 0o700;
pub const S_IRUSR: u32 = 0o400;
pub const S_IWUSR: u32 = 0o200;
pub const S_IXUSR: u32 = 0o100;
pub const S_IRWXG: u32 = 0o070;
pub const S_IRGRP: u32 = 0o040;
pub const S_IWGRP: u32 = 0o020;
pub const S_IXGRP: u32 = 0o010;
pub const S_IRWXO: u32 = 0o007;
pub const S_IROTH: u32 = 0o004;
pub const S_IWOTH: u32 = 0o002;
pub const S_IXOTH: u32 = 0o001;

/// Metadata for a file or directory.
///
/// Returned by [`FileSystem::stat`](crate::filesystem::FileSystem::stat) and
/// [`FileSystem::read_dir`](crate::filesystem::FileSystem::read_dir). Use
/// [`FileMetadata::file`](FileMetadata::file), [`FileMetadata::directory`](FileMetadata::directory),
/// or [`FileMetadata::new`](FileMetadata::new) to construct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileMetadata {
    /// Full path to the file.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// File mode (permissions + file type).
    pub mode: u32,
    /// Creation time.
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    pub created: Option<DateTime<Utc>>,
    /// Last modification time.
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    pub modified: Option<DateTime<Utc>>,
    /// Last access time.
    #[serde(with = "chrono::serde::ts_milliseconds_option")]
    pub accessed: Option<DateTime<Utc>>,
    /// Whether this is a symbolic link.
    pub is_symlink: bool,
    /// If this is a symlink, the target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<PathBuf>,
}

impl FileMetadata {
    /// Create new file metadata.
    pub fn new(path: PathBuf, mode: u32) -> Self {
        Self {
            path,
            size: 0,
            mode,
            created: None,
            modified: None,
            accessed: None,
            is_symlink: false,
            symlink_target: None,
        }
    }

    /// Create metadata for a regular file.
    pub fn file(path: PathBuf, size: u64) -> Self {
        Self {
            path: path.clone(),
            size,
            mode: S_IFREG | S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH,
            created: Some(Utc::now()),
            modified: Some(Utc::now()),
            accessed: Some(Utc::now()),
            is_symlink: false,
            symlink_target: None,
        }
    }

    /// Create metadata for a directory.
    pub fn directory(path: PathBuf) -> Self {
        Self {
            path,
            size: 0,
            mode: S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH,
            created: Some(Utc::now()),
            modified: Some(Utc::now()),
            accessed: Some(Utc::now()),
            is_symlink: false,
            symlink_target: None,
        }
    }

    /// Create metadata for a symlink.
    pub fn symlink(path: PathBuf, target: PathBuf) -> Self {
        Self {
            path,
            size: target.as_os_str().len() as u64,
            mode: S_IFLNK | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH,
            created: Some(Utc::now()),
            modified: Some(Utc::now()),
            accessed: Some(Utc::now()),
            is_symlink: true,
            symlink_target: Some(target),
        }
    }

    /// Check if this is a regular file.
    pub fn is_file(&self) -> bool {
        (self.mode & S_IFMT) == S_IFREG
    }

    /// Check if this is a directory.
    pub fn is_directory(&self) -> bool {
        (self.mode & S_IFMT) == S_IFDIR
    }

    /// Check if this is a symlink.
    pub fn is_symlink(&self) -> bool {
        self.is_symlink || (self.mode & S_IFMT) == S_IFLNK
    }

    /// Get directory mode constant.
    pub fn dir_mode() -> u32 {
        S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH
    }

    /// Get file mode constant.
    pub fn file_mode() -> u32 {
        S_IFREG | S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH
    }

    /// Get the permission bits (excluding file type).
    pub fn permissions(&self) -> u32 {
        self.mode & 0o777
    }

    /// Set the permission bits.
    pub fn set_permissions(&mut self, mode: u32) {
        self.mode = (self.mode & S_IFMT) | (mode & 0o777);
    }

    /// Get the file type as a string.
    pub fn file_type(&self) -> &str {
        match self.mode & S_IFMT {
            S_IFREG => "file",
            S_IFDIR => "directory",
            S_IFLNK => "symlink",
            S_IFCHR => "character device",
            S_IFBLK => "block device",
            S_IFIFO => "FIFO",
            S_IFSOCK => "socket",
            _ => "unknown",
        }
    }
}

impl fmt::Display for FileMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {} {}",
            self.file_type(),
            self.permissions(),
            self.size,
            self.modified
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            self.path.display()
        )
    }
}

/// Trait for systems that can provide file metadata.
pub trait MetadataProvider {
    fn metadata(&self, path: &Path) -> Result<FileMetadata, super::FSError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_file_metadata_new() {
        let path = PathBuf::from("/test/file.txt");
        let metadata = FileMetadata::new(path.clone(), S_IFREG | S_IRUSR | S_IWUSR);

        assert_eq!(metadata.path, path);
        assert_eq!(metadata.size, 0);
        assert_eq!(metadata.mode, S_IFREG | S_IRUSR | S_IWUSR);
        assert!(metadata.created.is_none());
        assert!(metadata.modified.is_none());
        assert!(metadata.accessed.is_none());
        assert!(!metadata.is_symlink);
        assert!(metadata.symlink_target.is_none());
    }

    #[test]
    fn test_file_metadata_file() {
        let path = PathBuf::from("/test/file.txt");
        let metadata = FileMetadata::file(path.clone(), 1024);

        assert_eq!(metadata.path, path);
        assert_eq!(metadata.size, 1024);
        assert!(metadata.is_file());
        assert!(!metadata.is_directory());
        assert!(!metadata.is_symlink());
    }

    #[test]
    fn test_file_metadata_directory() {
        let path = PathBuf::from("/test/dir");
        let metadata = FileMetadata::directory(path.clone());

        assert_eq!(metadata.path, path);
        assert_eq!(metadata.size, 0);
        assert!(!metadata.is_file());
        assert!(metadata.is_directory());
        assert!(!metadata.is_symlink());
    }

    #[test]
    fn test_file_metadata_symlink() {
        let path = PathBuf::from("/test/link");
        let target = PathBuf::from("/test/target");
        let metadata = FileMetadata::symlink(path.clone(), target.clone());

        assert_eq!(metadata.path, path);
        assert_eq!(metadata.size, target.as_os_str().len() as u64);
        assert!(!metadata.is_file());
        assert!(!metadata.is_directory());
        assert!(metadata.is_symlink());
        assert_eq!(metadata.symlink_target, Some(target));
    }

    #[test]
    fn test_is_file() {
        let mut metadata = FileMetadata::new(PathBuf::from("/test"), S_IFREG | S_IRUSR);
        assert!(metadata.is_file());

        metadata.mode = S_IFDIR | S_IRWXU;
        assert!(!metadata.is_file());
    }

    #[test]
    fn test_is_directory() {
        let mut metadata = FileMetadata::new(PathBuf::from("/test"), S_IFDIR | S_IRWXU);
        assert!(metadata.is_directory());

        metadata.mode = S_IFREG | S_IRUSR;
        assert!(!metadata.is_directory());
    }

    #[test]
    fn test_is_symlink() {
        let mut metadata = FileMetadata::new(PathBuf::from("/test"), S_IFLNK | S_IRWXU);
        assert!(metadata.is_symlink());

        // Test via is_symlink flag
        metadata.mode = S_IFREG | S_IRUSR;
        metadata.is_symlink = true;
        assert!(metadata.is_symlink());
    }

    #[test]
    fn test_dir_mode_and_file_mode() {
        let dir_mode = FileMetadata::dir_mode();
        assert_eq!(dir_mode & S_IFMT, S_IFDIR);
        assert!(dir_mode & S_IRWXU != 0);
        let file_mode = FileMetadata::file_mode();
        assert_eq!(file_mode & S_IFMT, S_IFREG);
        assert!(file_mode & S_IRUSR != 0);
    }

    #[test]
    fn test_permissions() {
        let metadata = FileMetadata::new(PathBuf::from("/test"), S_IFREG | S_IRWXU);
        assert_eq!(metadata.permissions(), S_IRWXU);

        let metadata2 = FileMetadata::new(
            PathBuf::from("/test"),
            S_IFREG | S_IRUSR | S_IWUSR | S_IRGRP,
        );
        assert_eq!(metadata2.permissions(), S_IRUSR | S_IWUSR | S_IRGRP);
    }

    #[test]
    fn test_set_permissions() {
        let mut metadata = FileMetadata::new(PathBuf::from("/test"), S_IFREG | S_IRWXU);
        metadata.set_permissions(S_IRUSR | S_IWUSR);

        assert_eq!(metadata.mode, S_IFREG | S_IRUSR | S_IWUSR);
    }

    #[test]
    fn test_file_type() {
        let file_meta = FileMetadata::new(PathBuf::from("/test"), S_IFREG | S_IRUSR);
        assert_eq!(file_meta.file_type(), "file");

        let dir_meta = FileMetadata::new(PathBuf::from("/test"), S_IFDIR | S_IRWXU);
        assert_eq!(dir_meta.file_type(), "directory");

        let symlink_meta = FileMetadata::new(PathBuf::from("/test"), S_IFLNK | S_IRWXU);
        assert_eq!(symlink_meta.file_type(), "symlink");

        let chr_meta = FileMetadata::new(PathBuf::from("/test"), S_IFCHR | S_IRWXU);
        assert_eq!(chr_meta.file_type(), "character device");

        let blk_meta = FileMetadata::new(PathBuf::from("/test"), S_IFBLK | S_IRWXU);
        assert_eq!(blk_meta.file_type(), "block device");

        let fifo_meta = FileMetadata::new(PathBuf::from("/test"), S_IFIFO | S_IRWXU);
        assert_eq!(fifo_meta.file_type(), "FIFO");

        let sock_meta = FileMetadata::new(PathBuf::from("/test"), S_IFSOCK | S_IRWXU);
        assert_eq!(sock_meta.file_type(), "socket");

        // Unrecognized file type falls back to "unknown"
        let unknown_meta = FileMetadata::new(PathBuf::from("/test"), 0o030000 | S_IRWXU);
        assert_eq!(unknown_meta.file_type(), "unknown");
    }

    #[test]
    fn test_display() {
        let metadata = FileMetadata::file(PathBuf::from("/test/file.txt"), 1024);
        let display = metadata.to_string();

        assert!(display.contains("file"));
        assert!(display.contains("1024"));
        assert!(display.contains("/test/file.txt"));
    }

    #[test]
    fn test_serialize_file_metadata() {
        let metadata = FileMetadata::file(PathBuf::from("/test/file.txt"), 1024);
        let json = serde_json::to_string(&metadata).expect("Failed to serialize");

        assert!(json.contains("\"path\""));
        assert!(json.contains("\"size\":1024"));
        assert!(json.contains("\"mode\""));
    }

    #[test]
    fn test_deserialize_file_metadata() {
        let json = r#"{
            "path": "/test/file.txt",
            "size": 1024,
            "mode": 33184,
            "created": 1609459200000,
            "modified": 1609459200000,
            "accessed": 1609459200000,
            "is_symlink": false
        }"#;

        let metadata: FileMetadata = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(metadata.path, PathBuf::from("/test/file.txt"));
        assert_eq!(metadata.size, 1024);
        assert!(!metadata.is_symlink);
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let original = FileMetadata::file(PathBuf::from("/test/file.txt"), 2048);

        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let decoded: FileMetadata = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(decoded.path, original.path);
        assert_eq!(decoded.size, original.size);
        assert_eq!(decoded.mode, original.mode);
        assert_eq!(decoded.is_symlink, original.is_symlink);
    }

    #[test]
    fn test_symlink_target_not_serialized_when_none() {
        let metadata = FileMetadata::file(PathBuf::from("/test/file.txt"), 1024);
        let json = serde_json::to_string(&metadata).expect("Failed to serialize");

        assert!(!json.contains("symlink_target"));
    }

    #[test]
    fn test_symlink_target_serialized_when_some() {
        let metadata =
            FileMetadata::symlink(PathBuf::from("/test/link"), PathBuf::from("/test/target"));
        let json = serde_json::to_string(&metadata).expect("Failed to serialize");

        assert!(json.contains("symlink_target"));
        assert!(json.contains("target"));
    }

    #[test]
    fn test_constants_exist() {
        // Verify constants are properly defined
        assert_eq!(S_IFMT, 0o170000);
        assert_eq!(S_IFREG, 0o100000);
        assert_eq!(S_IFDIR, 0o040000);
        assert_eq!(S_IFLNK, 0o120000);
        assert_eq!(S_IRWXU, 0o700);
        assert_eq!(S_IRUSR, 0o400);
        assert_eq!(S_IWUSR, 0o200);
        assert_eq!(S_IXUSR, 0o100);
    }
}
