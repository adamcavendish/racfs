//! Error types for RACFS filesystem operations.

use std::{path::PathBuf, string::FromUtf8Error};

use snafu::Snafu;

/// Errors that can occur during filesystem operations.
///
/// Used as the error type for filesystem `Result<T, FSError>`. Convertible from `std::io::Error` and
/// `FromUtf8Error`. Common variants: `NotFound`, `PermissionDenied`, `AlreadyExists`,
/// `ReadOnly`, `NotSupported`, `Io`.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub enum FSError {
    #[snafu(display("path not found: {}", path.display()))]
    NotFound { path: PathBuf },

    #[snafu(display("permission denied: {}", path.display()))]
    PermissionDenied { path: PathBuf },

    #[snafu(display("path already exists: {}", path.display()))]
    AlreadyExists { path: PathBuf },

    #[snafu(display("path is a directory: {}", path.display()))]
    IsDirectory { path: PathBuf },

    #[snafu(display("path is not a directory: {}", path.display()))]
    NotADirectory { path: PathBuf },

    #[snafu(display("invalid input: {}", message))]
    InvalidInput { message: String },

    #[snafu(display("I/O error: {}", message))]
    Io { message: String },

    #[snafu(display("operation not supported: {}", message))]
    NotSupported { message: String },

    #[snafu(display("handle already in use: {}", handle_id))]
    AlreadyInUse { handle_id: String },

    #[snafu(display("invalid handle: {}", handle_id))]
    InvalidHandle { handle_id: String },

    #[snafu(display("out of memory"))]
    OutOfMemory,

    #[snafu(display("path too long"))]
    PathTooLong,

    #[snafu(display("filename too long"))]
    FilenameTooLong,

    #[snafu(display("read-only filesystem"))]
    ReadOnly,

    #[snafu(display("storage full"))]
    StorageFull,

    #[snafu(display("too many open files"))]
    TooManyOpenFiles,

    #[snafu(display("directory not empty"))]
    DirectoryNotEmpty,

    #[snafu(display("cross-device link not allowed"))]
    CrossDeviceLink,

    #[snafu(display("invalid UTF-8: {}", message))]
    InvalidUtf8 { message: String },

    #[snafu(display("timeout"))]
    Timeout,
}

impl From<std::io::Error> for FSError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => FSError::NotFound {
                path: PathBuf::from(""),
            },
            std::io::ErrorKind::PermissionDenied => FSError::PermissionDenied {
                path: PathBuf::from(""),
            },
            std::io::ErrorKind::AlreadyExists => FSError::AlreadyExists {
                path: PathBuf::from(""),
            },
            std::io::ErrorKind::IsADirectory => FSError::IsDirectory {
                path: PathBuf::from(""),
            },
            std::io::ErrorKind::NotADirectory => FSError::NotADirectory {
                path: PathBuf::from(""),
            },
            std::io::ErrorKind::InvalidInput => FSError::InvalidInput {
                message: err.to_string(),
            },
            std::io::ErrorKind::Unsupported => FSError::NotSupported {
                message: err.to_string(),
            },
            std::io::ErrorKind::OutOfMemory => FSError::OutOfMemory,
            std::io::ErrorKind::FileTooLarge => FSError::InvalidInput {
                message: "file too large".to_string(),
            },
            std::io::ErrorKind::DirectoryNotEmpty => FSError::DirectoryNotEmpty,
            std::io::ErrorKind::ReadOnlyFilesystem => FSError::ReadOnly,
            std::io::ErrorKind::StorageFull => FSError::StorageFull,
            _ => FSError::Io {
                message: err.to_string(),
            },
        }
    }
}

impl From<FromUtf8Error> for FSError {
    fn from(err: FromUtf8Error) -> Self {
        FSError::InvalidUtf8 {
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_not_found_error() {
        let path = PathBuf::from("/nonexistent");
        let error = FSError::NotFound { path: path.clone() };
        assert!(matches!(error, FSError::NotFound { .. }));
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn test_already_exists_error() {
        let path = PathBuf::from("/existing");
        let error = FSError::AlreadyExists { path: path.clone() };
        assert!(matches!(error, FSError::AlreadyExists { .. }));
        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn test_permission_denied_error() {
        let path = PathBuf::from("/denied");
        let error = FSError::PermissionDenied { path: path.clone() };
        assert!(matches!(error, FSError::PermissionDenied { .. }));
        assert!(error.to_string().contains("permission denied"));
    }

    #[test]
    fn test_is_directory_error() {
        let path = PathBuf::from("/dir");
        let error = FSError::IsDirectory { path: path.clone() };
        assert!(matches!(error, FSError::IsDirectory { .. }));
        assert!(error.to_string().contains("is a directory"));
    }

    #[test]
    fn test_not_a_directory_error() {
        let path = PathBuf::from("/file");
        let error = FSError::NotADirectory { path: path.clone() };
        assert!(matches!(error, FSError::NotADirectory { .. }));
        assert!(error.to_string().contains("not a directory"));
    }

    #[test]
    fn test_invalid_input_error() {
        let message = "invalid argument".to_string();
        let error = FSError::InvalidInput {
            message: message.clone(),
        };
        assert!(matches!(error, FSError::InvalidInput { .. }));
        assert!(error.to_string().contains("invalid input"));
    }

    #[test]
    fn test_io_error() {
        let message = "I/O error occurred".to_string();
        let error = FSError::Io {
            message: message.clone(),
        };
        assert!(matches!(error, FSError::Io { .. }));
        assert!(error.to_string().contains("I/O error"));
    }

    #[test]
    fn test_not_supported_error() {
        let message = "operation not supported".to_string();
        let error = FSError::NotSupported {
            message: message.clone(),
        };
        assert!(matches!(error, FSError::NotSupported { .. }));
        assert!(error.to_string().contains("not supported"));
    }

    #[test]
    fn test_already_in_use_error() {
        let handle_id = "handle123".to_string();
        let error = FSError::AlreadyInUse {
            handle_id: handle_id.clone(),
        };
        assert!(matches!(error, FSError::AlreadyInUse { .. }));
        assert!(error.to_string().contains("already in use"));
    }

    #[test]
    fn test_invalid_handle_error() {
        let handle_id = "invalid_handle".to_string();
        let error = FSError::InvalidHandle {
            handle_id: handle_id.clone(),
        };
        assert!(matches!(error, FSError::InvalidHandle { .. }));
        assert!(error.to_string().contains("invalid handle"));
    }

    #[test]
    fn test_out_of_memory_error() {
        let error = FSError::OutOfMemory;
        assert!(matches!(error, FSError::OutOfMemory));
        assert!(error.to_string().contains("out of memory"));
    }

    #[test]
    fn test_path_too_long_error() {
        let error = FSError::PathTooLong;
        assert!(matches!(error, FSError::PathTooLong));
        assert!(error.to_string().contains("path too long"));
    }

    #[test]
    fn test_filename_too_long_error() {
        let error = FSError::FilenameTooLong;
        assert!(matches!(error, FSError::FilenameTooLong));
        assert!(error.to_string().contains("filename too long"));
    }

    #[test]
    fn test_read_only_error() {
        let error = FSError::ReadOnly;
        assert!(matches!(error, FSError::ReadOnly));
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn test_storage_full_error() {
        let error = FSError::StorageFull;
        assert!(matches!(error, FSError::StorageFull));
        assert!(error.to_string().contains("storage full"));
    }

    #[test]
    fn test_too_many_open_files_error() {
        let error = FSError::TooManyOpenFiles;
        assert!(matches!(error, FSError::TooManyOpenFiles));
        assert!(error.to_string().contains("too many open files"));
    }

    #[test]
    fn test_directory_not_empty_error() {
        let error = FSError::DirectoryNotEmpty;
        assert!(matches!(error, FSError::DirectoryNotEmpty));
        assert!(error.to_string().contains("directory not empty"));
    }

    #[test]
    fn test_cross_device_link_error() {
        let error = FSError::CrossDeviceLink;
        assert!(matches!(error, FSError::CrossDeviceLink));
        assert!(error.to_string().contains("cross-device"));
    }

    #[test]
    fn test_invalid_utf8_error() {
        let message = "invalid UTF-8".to_string();
        let error = FSError::InvalidUtf8 {
            message: message.clone(),
        };
        assert!(matches!(error, FSError::InvalidUtf8 { .. }));
        assert!(error.to_string().contains("invalid UTF-8"));
    }

    #[test]
    fn test_timeout_error() {
        let error = FSError::Timeout;
        assert!(matches!(error, FSError::Timeout));
        assert!(error.to_string().contains("timeout"));
    }

    #[test]
    fn test_from_io_not_found() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::NotFound { .. }));
    }

    #[test]
    fn test_from_io_permission_denied() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::PermissionDenied { .. }));
    }

    #[test]
    fn test_from_io_already_exists() {
        let io_err = io::Error::new(io::ErrorKind::AlreadyExists, "file exists");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::AlreadyExists { .. }));
    }

    #[test]
    fn test_from_io_is_directory() {
        let io_err = io::Error::new(io::ErrorKind::IsADirectory, "is a directory");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::IsDirectory { .. }));
    }

    #[test]
    fn test_from_io_not_a_directory() {
        let io_err = io::Error::new(io::ErrorKind::NotADirectory, "not a directory");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::NotADirectory { .. }));
    }

    #[test]
    fn test_from_io_invalid_input() {
        let io_err = io::Error::new(io::ErrorKind::InvalidInput, "invalid input");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::InvalidInput { .. }));
    }

    #[test]
    fn test_from_io_out_of_memory() {
        let io_err = io::Error::new(io::ErrorKind::OutOfMemory, "out of memory");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::OutOfMemory));
    }

    #[test]
    fn test_from_io_directory_not_empty() {
        let io_err = io::Error::new(io::ErrorKind::DirectoryNotEmpty, "not empty");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::DirectoryNotEmpty));
    }

    #[test]
    fn test_from_io_read_only() {
        let io_err = io::Error::new(io::ErrorKind::ReadOnlyFilesystem, "read only");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::ReadOnly));
    }

    #[test]
    fn test_from_io_storage_full() {
        let io_err = io::Error::new(io::ErrorKind::StorageFull, "no space left");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::StorageFull));
    }

    #[test]
    fn test_from_io_unsupported() {
        let io_err = io::Error::new(io::ErrorKind::Unsupported, "unsupported");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::NotSupported { .. }));
    }

    #[test]
    fn test_from_io_file_too_large() {
        let io_err = io::Error::new(io::ErrorKind::FileTooLarge, "file too large");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::InvalidInput { .. }));
        assert!(fs_error.to_string().to_lowercase().contains("large"));
    }

    #[test]
    fn test_from_io_falls_back_to_io_error() {
        let io_err = io::Error::other("other error");
        let fs_error = FSError::from(io_err);
        assert!(matches!(fs_error, FSError::Io { .. }));
    }

    #[test]
    fn test_from_utf8_error() {
        let invalid = b"\xff\xfe".to_vec();
        let utf8_err = String::from_utf8(invalid).unwrap_err();
        let fs_error = FSError::from(utf8_err);
        assert!(matches!(fs_error, FSError::InvalidUtf8 { .. }));
        assert!(fs_error.to_string().to_lowercase().contains("utf"));
    }
}
