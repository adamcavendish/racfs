//! Core filesystem trait and extensions.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::error::FSError;
use crate::file_handle::FileHandle;
use crate::flags::{OpenFlags, WriteFlags};
use crate::metadata::FileMetadata;

// -----------------------------------------------------------------------------
// Subtraits: implement these to get FileSystem via blanket impl
// -----------------------------------------------------------------------------

/// Read operations: read file content and stat.
#[async_trait]
pub trait ReadFS: Send + Sync {
    /// Read data from a file.
    ///
    /// - `offset`: Starting position in file.
    /// - `size`: Maximum bytes to read (-1 for all remaining).
    /// Returns up to `size` bytes starting at offset.
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError>;

    /// Get file or directory metadata.
    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError>;
}

/// Write operations: create file and write content.
#[async_trait]
pub trait WriteFS: Send + Sync {
    /// Create a new empty file.
    ///
    /// Returns error if file already exists or parent doesn't exist.
    async fn create(&self, path: &Path) -> Result<(), FSError>;

    /// Write data to a file.
    ///
    /// - `data`: Bytes to write.
    /// - `offset`: Starting position in file.
    /// - `flags`: Write behavior flags.
    /// Returns number of bytes written.
    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError>;
}

/// Directory and path operations: mkdir, read_dir, remove, rename.
#[async_trait]
pub trait DirFS: Send + Sync {
    /// Create a directory.
    ///
    /// Returns error if directory already exists or parent doesn't exist.
    async fn mkdir(&self, path: &Path, perm: u32) -> Result<(), FSError>;

    /// List directory contents.
    ///
    /// Returns list of entries in the directory.
    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError>;

    /// Remove a file or empty directory.
    ///
    /// Returns error if path doesn't exist or is a non-empty directory.
    async fn remove(&self, path: &Path) -> Result<(), FSError>;

    /// Remove a file or directory recursively.
    ///
    /// Removes all contents of the directory.
    async fn remove_all(&self, path: &Path) -> Result<(), FSError>;

    /// Rename or move a file or directory.
    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError>;
}

/// Permission change operation.
#[async_trait]
pub trait ChmodFS: Send + Sync {
    /// Change file permissions.
    async fn chmod(&self, path: &Path, mode: u32) -> Result<(), FSError>;
}

// -----------------------------------------------------------------------------
// FileSystem: composition of subtraits + optional operations with defaults
// -----------------------------------------------------------------------------

/// Core filesystem trait that all VFS backends must implement.
///
/// Implement by implementing [`ReadFS`], [`WriteFS`], [`DirFS`], and [`ChmodFS`];
/// the blanket impl then provides [`FileSystem`] and default implementations
/// for truncate, touch, readlink, symlink, and xattr (returning NotSupported).
#[async_trait]
pub trait FileSystem: ReadFS + WriteFS + DirFS + ChmodFS + Send + Sync {
    /// Truncate a file to specified size.
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let _ = (path, size);
        Err(FSError::NotSupported {
            message: "truncate not implemented".to_string(),
        })
    }

    /// Touch a file (update access and modification times).
    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        let _ = path;
        Err(FSError::NotSupported {
            message: "touch not implemented".to_string(),
        })
    }

    /// Read the target of a symbolic link.
    async fn readlink(&self, path: &Path) -> Result<PathBuf, FSError> {
        let _ = path;
        Err(FSError::NotSupported {
            message: "readlink not implemented".to_string(),
        })
    }

    /// Create a symbolic link at `link` pointing to `target`.
    async fn symlink(&self, target: &Path, link: &Path) -> Result<(), FSError> {
        let _ = (target, link);
        Err(FSError::NotSupported {
            message: "symlink not implemented".to_string(),
        })
    }

    /// Get an extended attribute value.
    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Vec<u8>, FSError> {
        let _ = (path, name);
        Err(FSError::NotSupported {
            message: "get_xattr not implemented".to_string(),
        })
    }

    /// Set an extended attribute.
    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<(), FSError> {
        let _ = (path, name, value);
        Err(FSError::NotSupported {
            message: "set_xattr not implemented".to_string(),
        })
    }

    /// Remove an extended attribute.
    async fn remove_xattr(&self, path: &Path, name: &str) -> Result<(), FSError> {
        let _ = (path, name);
        Err(FSError::NotSupported {
            message: "remove_xattr not implemented".to_string(),
        })
    }

    /// List extended attribute names.
    async fn list_xattr(&self, path: &Path) -> Result<Vec<String>, FSError> {
        let _ = path;
        Err(FSError::NotSupported {
            message: "list_xattr not implemented".to_string(),
        })
    }
}

// -----------------------------------------------------------------------------
// Extension traits (unchanged; still require FileSystem)
// -----------------------------------------------------------------------------
#[async_trait]
pub trait HandleFS: FileSystem {
    /// Open a file and return a handle.
    async fn open(&self, path: &Path, flags: OpenFlags) -> Result<FileHandle, FSError>;

    /// Close an open handle.
    async fn close(&self, handle_id: &crate::file_handle::HandleId) -> Result<(), FSError>;
}

/// Extension trait for symlink operations.
#[async_trait]
pub trait Symlinker: FileSystem {
    /// Create a symbolic link.
    async fn symlink(&self, target: &Path, link: &Path) -> Result<(), FSError>;

    /// Read the target of a symbolic link.
    async fn readlink(&self, path: &Path) -> Result<PathBuf, FSError>;
}

/// Extension trait for file touching.
#[async_trait]
pub trait Toucher: FileSystem {
    /// Touch a file (create if not exists, update times if exists).
    async fn touch(&self, path: &Path) -> Result<(), FSError>;
}

/// Extension trait for streaming operations.
#[async_trait]
pub trait Streamer: FileSystem {
    /// Open a file for streaming read.
    /// Returns a reader that can be used to read the file content.
    async fn read_stream(
        &self,
        path: &Path,
        offset: i64,
        size: i64,
    ) -> Result<Box<dyn std::io::Read + Send + Sync>, FSError>;

    /// Open a file for streaming write.
    /// Returns a writer that can be used to write file content.
    async fn write_stream(
        &self,
        path: &Path,
        offset: i64,
    ) -> Result<Box<dyn std::io::Write + Send + Sync>, FSError>;
}

/// Extension trait for extended attribute operations.
#[async_trait]
pub trait XattrFS: FileSystem {
    /// Get an extended attribute.
    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Vec<u8>, FSError>;

    /// Set an extended attribute.
    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<(), FSError>;

    /// Remove an extended attribute.
    async fn remove_xattr(&self, path: &Path, name: &str) -> Result<(), FSError>;

    /// List extended attributes.
    async fn list_xattr(&self, path: &Path) -> Result<Vec<String>, FSError>;
}

/// Extension trait for hard link operations.
#[async_trait]
pub trait Linker: FileSystem {
    /// Create a hard link.
    async fn link(&self, target: &Path, link: &Path) -> Result<(), FSError>;
}

/// Extension trait for absolute path operations.
#[async_trait]
pub trait RealPathFS: FileSystem {
    /// Resolve a path to its absolute canonical form.
    async fn realpath(&self, path: &Path) -> Result<PathBuf, FSError>;
}

/// Utility functions for filesystem implementations.
pub mod utils {
    use std::path::{Path, PathBuf};

    /// Normalize a path by removing . and .. components.
    pub fn normalize_path(path: &Path) -> std::path::PathBuf {
        let mut result = std::path::PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    result.pop();
                }
                std::path::Component::CurDir => {}
                std::path::Component::RootDir => result.push("/"),
                std::path::Component::Normal(name) => result.push(name),
                std::path::Component::Prefix(prefix) => {
                    result.push(prefix.as_os_str());
                }
            }
        }
        result
    }

    /// Get the parent directory of a path.
    pub fn parent(path: &Path) -> Option<PathBuf> {
        path.parent().map(|p| p.to_path_buf())
    }

    /// Get the filename from a path.
    pub fn filename(path: &Path) -> Option<std::ffi::OsString> {
        path.file_name().map(|n| n.to_os_string())
    }

    /// Check if a path is absolute.
    pub fn is_absolute(path: &Path) -> bool {
        path.is_absolute()
    }

    /// Check if a path is relative.
    pub fn is_relative(path: &Path) -> bool {
        path.is_relative()
    }

    /// Join path components.
    pub fn join<'a>(base: &'a Path, parts: &[&str]) -> std::borrow::Cow<'a, Path> {
        let mut result = base.to_path_buf();
        for part in parts {
            result.push(part);
        }
        std::borrow::Cow::Owned(result)
    }
}

#[cfg(test)]
mod utils_tests {
    use super::utils;
    use std::path::Path;

    #[test]
    fn normalize_path_removes_dot() {
        let p = Path::new("/a/./b");
        let n = utils::normalize_path(p);
        assert_eq!(n.to_string_lossy(), "/a/b");
    }

    #[test]
    fn normalize_path_removes_dotdot() {
        let p = Path::new("/a/b/../c");
        let n = utils::normalize_path(p);
        assert_eq!(n.to_string_lossy(), "/a/c");
    }

    #[test]
    fn parent_returns_parent() {
        assert_eq!(
            utils::parent(Path::new("/a/b")).map(|p| p.to_string_lossy().into_owned()),
            Some("/a".to_string())
        );
    }

    #[test]
    fn filename_returns_last_component() {
        assert_eq!(
            utils::filename(Path::new("/a/b.txt")).map(|o| o.to_string_lossy().into_owned()),
            Some("b.txt".to_string())
        );
    }

    #[test]
    fn is_absolute() {
        assert!(utils::is_absolute(Path::new("/foo")));
        assert!(!utils::is_absolute(Path::new("foo")));
    }

    #[test]
    fn is_relative() {
        assert!(utils::is_relative(Path::new("foo")));
        assert!(!utils::is_relative(Path::new("/foo")));
    }

    #[test]
    fn join_components() {
        let base = Path::new("/base");
        let j = utils::join(base, &["a", "b"]);
        assert_eq!(j.to_string_lossy(), "/base/a/b");
    }

    #[test]
    fn normalize_path_root() {
        let p = Path::new("/");
        let n = utils::normalize_path(p);
        assert_eq!(n.to_string_lossy(), "/");
    }

    #[test]
    fn normalize_path_empty_relative() {
        let p = Path::new(".");
        let n = utils::normalize_path(p);
        assert_eq!(n.to_string_lossy(), "");
    }

    #[test]
    fn parent_of_root_is_none() {
        assert_eq!(utils::parent(Path::new("/")), None);
    }

    #[test]
    fn join_empty_parts() {
        let base = Path::new("/base");
        let j = utils::join(base, &[]);
        assert_eq!(j.to_string_lossy(), "/base");
    }
}

#[cfg(test)]
mod default_fs_tests {
    use super::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};
    use crate::error::FSError;
    use crate::flags::WriteFlags;
    use crate::metadata::FileMetadata;
    use async_trait::async_trait;
    use std::path::Path;

    struct StubFS;

    #[async_trait]
    impl ReadFS for StubFS {
        async fn read(&self, _path: &Path, _offset: i64, _size: i64) -> Result<Vec<u8>, FSError> {
            Err(FSError::NotSupported {
                message: "read".to_string(),
            })
        }
        async fn stat(&self, _path: &Path) -> Result<FileMetadata, FSError> {
            Err(FSError::NotSupported {
                message: "stat".to_string(),
            })
        }
    }

    #[async_trait]
    impl WriteFS for StubFS {
        async fn create(&self, _path: &Path) -> Result<(), FSError> {
            Err(FSError::NotSupported {
                message: "create".to_string(),
            })
        }
        async fn write(
            &self,
            _path: &Path,
            _data: &[u8],
            _offset: i64,
            _flags: WriteFlags,
        ) -> Result<u64, FSError> {
            Err(FSError::NotSupported {
                message: "write".to_string(),
            })
        }
    }

    #[async_trait]
    impl DirFS for StubFS {
        async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
            Err(FSError::NotSupported {
                message: "mkdir".to_string(),
            })
        }
        async fn read_dir(&self, _path: &Path) -> Result<Vec<FileMetadata>, FSError> {
            Err(FSError::NotSupported {
                message: "read_dir".to_string(),
            })
        }
        async fn remove(&self, _path: &Path) -> Result<(), FSError> {
            Err(FSError::NotSupported {
                message: "remove".to_string(),
            })
        }
        async fn remove_all(&self, _path: &Path) -> Result<(), FSError> {
            Err(FSError::NotSupported {
                message: "remove_all".to_string(),
            })
        }
        async fn rename(&self, _old: &Path, _new: &Path) -> Result<(), FSError> {
            Err(FSError::NotSupported {
                message: "rename".to_string(),
            })
        }
    }

    #[async_trait]
    impl ChmodFS for StubFS {
        async fn chmod(&self, _path: &Path, _mode: u32) -> Result<(), FSError> {
            Err(FSError::NotSupported {
                message: "chmod".to_string(),
            })
        }
    }

    impl FileSystem for StubFS {}

    #[tokio::test]
    async fn default_truncate_returns_not_supported() {
        let fs = StubFS;
        let err = fs.truncate(Path::new("/f"), 0).await.unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("truncate"));
    }

    #[tokio::test]
    async fn default_touch_returns_not_supported() {
        let fs = StubFS;
        let err = fs.touch(Path::new("/f")).await.unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("touch"));
    }

    #[tokio::test]
    async fn default_readlink_returns_not_supported() {
        let fs = StubFS;
        let err = fs.readlink(Path::new("/link")).await.unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("readlink"));
    }

    #[tokio::test]
    async fn default_symlink_returns_not_supported() {
        let fs = StubFS;
        let err = fs
            .symlink(Path::new("/t"), Path::new("/link"))
            .await
            .unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("symlink"));
    }

    #[tokio::test]
    async fn default_get_xattr_returns_not_supported() {
        let fs = StubFS;
        let err = fs.get_xattr(Path::new("/f"), "user.x").await.unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("get_xattr"));
    }

    #[tokio::test]
    async fn default_set_xattr_returns_not_supported() {
        let fs = StubFS;
        let err = fs
            .set_xattr(Path::new("/f"), "user.x", b"v")
            .await
            .unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("set_xattr"));
    }

    #[tokio::test]
    async fn default_remove_xattr_returns_not_supported() {
        let fs = StubFS;
        let err = fs
            .remove_xattr(Path::new("/f"), "user.x")
            .await
            .unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("remove_xattr"));
    }

    #[tokio::test]
    async fn default_list_xattr_returns_not_supported() {
        let fs = StubFS;
        let err = fs.list_xattr(Path::new("/f")).await.unwrap_err();
        assert!(matches!(err, FSError::NotSupported { .. }));
        assert!(err.to_string().contains("list_xattr"));
    }
}
