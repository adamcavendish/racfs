//! Error handling utilities for the server

use racfs_core::error::FSError;
use racfs_http_error::{ErrorCode, HttpErrorResponse};

/// Maps FSError to HttpErrorResponse
#[allow(dead_code)]
pub fn map_fs_error(error: FSError) -> HttpErrorResponse {
    map_fs_error_with_context(error, None, None)
}

/// Maps FSError to HttpErrorResponse with optional operation and path context
pub fn map_fs_error_with_context(
    error: FSError,
    operation: Option<&str>,
    path: Option<&std::path::Path>,
) -> HttpErrorResponse {
    let (code, message) = match &error {
        FSError::NotFound { path } => (
            ErrorCode::NotFound,
            format!("Not found: {}", path.display()),
        ),
        FSError::AlreadyExists { path } => (
            ErrorCode::Conflict,
            format!("Already exists: {}", path.display()),
        ),
        FSError::PermissionDenied { path } => (
            ErrorCode::Forbidden,
            format!("Permission denied: {}", path.display()),
        ),
        FSError::IsDirectory { path } => (
            ErrorCode::BadRequest,
            format!("Is a directory: {}", path.display()),
        ),
        FSError::NotADirectory { path } => (
            ErrorCode::BadRequest,
            format!("Not a directory: {}", path.display()),
        ),
        FSError::InvalidInput { message } => {
            (ErrorCode::BadRequest, format!("Invalid input: {}", message))
        }
        FSError::NotSupported { message } => (
            ErrorCode::NotImplemented,
            format!("Not supported: {}", message),
        ),
        FSError::AlreadyInUse { handle_id } => (
            ErrorCode::Conflict,
            format!("Already in use: {}", handle_id),
        ),
        FSError::InvalidHandle { handle_id } => (
            ErrorCode::BadRequest,
            format!("Invalid handle: {}", handle_id),
        ),
        FSError::OutOfMemory => (ErrorCode::ServiceUnavailable, "Out of memory".to_string()),
        FSError::PathTooLong => (ErrorCode::BadRequest, "Path too long".to_string()),
        FSError::FilenameTooLong => (ErrorCode::BadRequest, "Filename too long".to_string()),
        FSError::ReadOnly => (ErrorCode::Forbidden, "Read only".to_string()),
        FSError::StorageFull => (ErrorCode::InsufficientStorage, "Storage full".to_string()),
        FSError::TooManyOpenFiles => (
            ErrorCode::ServiceUnavailable,
            "Too many open files".to_string(),
        ),
        FSError::DirectoryNotEmpty => (ErrorCode::Conflict, "Directory not empty".to_string()),
        FSError::CrossDeviceLink => (ErrorCode::BadRequest, "Cross device link".to_string()),
        FSError::InvalidUtf8 { message } => {
            (ErrorCode::BadRequest, format!("Invalid UTF-8: {}", message))
        }
        FSError::Timeout => (ErrorCode::GatewayTimeout, "Operation timed out".to_string()),
        FSError::Io { message } => (
            ErrorCode::InternalServerError,
            format!("I/O error: {}", message),
        ),
    };

    let detail = match (operation, path) {
        (Some(op), Some(p)) => Some(format!("operation: {}, path: {}", op, p.display())),
        (Some(op), None) => Some(format!("operation: {}", op)),
        (None, Some(p)) => Some(format!("path: {}", p.display())),
        (None, None) => None,
    };

    match detail {
        Some(d) => HttpErrorResponse::new_with_detail(code, message, d),
        None => HttpErrorResponse::new(code, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use racfs_core::error::FSError;
    use racfs_http_error::ErrorCode;
    use std::path::PathBuf;

    #[test]
    fn map_fs_error_not_found() {
        let err = FSError::NotFound {
            path: PathBuf::from("/foo/bar"),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::NotFound);
        assert!(resp.message.contains("Not found"));
        assert!(resp.message.contains("/foo/bar"));
        assert!(resp.detail.is_none());
    }

    #[test]
    fn map_fs_error_already_exists() {
        let err = FSError::AlreadyExists {
            path: PathBuf::from("/exists"),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::Conflict);
        assert!(resp.message.contains("Already exists"));
        assert!(resp.detail.is_none());
    }

    #[test]
    fn map_fs_error_invalid_input() {
        let err = FSError::InvalidInput {
            message: "bad value".to_string(),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Invalid input"));
        assert!(resp.message.contains("bad value"));
    }

    #[test]
    fn map_fs_error_not_supported() {
        let err = FSError::NotSupported {
            message: "feature X".to_string(),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::NotImplemented);
        assert!(resp.message.contains("Not supported"));
    }

    #[test]
    fn map_fs_error_out_of_memory() {
        let err = FSError::OutOfMemory;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::ServiceUnavailable);
        assert_eq!(resp.message, "Out of memory");
    }

    #[test]
    fn map_fs_error_storage_full() {
        let err = FSError::StorageFull;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::InsufficientStorage);
        assert_eq!(resp.message, "Storage full");
    }

    #[test]
    fn map_fs_error_timeout() {
        let err = FSError::Timeout;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::GatewayTimeout);
        assert_eq!(resp.message, "Operation timed out");
    }

    #[test]
    fn map_fs_error_io() {
        let err = FSError::Io {
            message: "disk failed".to_string(),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::InternalServerError);
        assert!(resp.message.contains("I/O error"));
        assert!(resp.message.contains("disk failed"));
    }

    #[test]
    fn map_fs_error_with_context_both() {
        let err = FSError::NotFound {
            path: PathBuf::from("/a/b"),
        };
        let resp = map_fs_error_with_context(err, Some("read"), Some(std::path::Path::new("/a/b")));
        assert_eq!(resp.code, ErrorCode::NotFound);
        assert!(resp.message.contains("Not found"));
        let detail = resp.detail.expect("detail set");
        assert!(detail.contains("operation: read"));
        assert!(detail.contains("path: /a/b"));
    }

    #[test]
    fn map_fs_error_with_context_operation_only() {
        let err = FSError::InvalidInput {
            message: "x".to_string(),
        };
        let resp = map_fs_error_with_context(err, Some("stat"), None);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(
            resp.detail
                .as_ref()
                .map(|d| d == "operation: stat")
                .unwrap_or(false)
        );
    }

    #[test]
    fn map_fs_error_with_context_path_only() {
        let err = FSError::PermissionDenied {
            path: PathBuf::from("/secret"),
        };
        let resp = map_fs_error_with_context(err, None, Some(std::path::Path::new("/secret")));
        assert_eq!(resp.code, ErrorCode::Forbidden);
        assert!(
            resp.detail
                .as_ref()
                .map(|d| d == "path: /secret")
                .unwrap_or(false)
        );
    }

    #[test]
    fn map_fs_error_with_context_none() {
        let err = FSError::ReadOnly;
        let resp = map_fs_error_with_context(err, None, None);
        assert_eq!(resp.code, ErrorCode::Forbidden);
        assert_eq!(resp.message, "Read only");
        assert!(resp.detail.is_none());
    }

    #[test]
    fn map_fs_error_display_impl() {
        let err = FSError::NotFound {
            path: PathBuf::from("/x"),
        };
        let resp = map_fs_error(err);
        let s = format!("{}", resp);
        assert!(s.contains("NotFound"));
        assert!(s.contains("Not found"));
    }

    #[test]
    fn map_fs_error_permission_denied() {
        let err = FSError::PermissionDenied {
            path: PathBuf::from("/secret"),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::Forbidden);
        assert!(resp.message.contains("Permission denied"));
        assert!(resp.message.contains("/secret"));
    }

    #[test]
    fn map_fs_error_is_directory() {
        let err = FSError::IsDirectory {
            path: PathBuf::from("/dir"),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Is a directory"));
    }

    #[test]
    fn map_fs_error_not_a_directory() {
        let err = FSError::NotADirectory {
            path: PathBuf::from("/file"),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Not a directory"));
    }

    #[test]
    fn map_fs_error_directory_not_empty() {
        let err = FSError::DirectoryNotEmpty;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::Conflict);
        assert!(resp.message.contains("Directory not empty"));
    }

    #[test]
    fn map_fs_error_already_in_use() {
        let err = FSError::AlreadyInUse {
            handle_id: "h1".to_string(),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::Conflict);
        assert!(resp.message.contains("Already in use"));
        assert!(resp.message.contains("h1"));
    }

    #[test]
    fn map_fs_error_invalid_handle() {
        let err = FSError::InvalidHandle {
            handle_id: "bad".to_string(),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Invalid handle"));
        assert!(resp.message.contains("bad"));
    }

    #[test]
    fn map_fs_error_path_too_long() {
        let err = FSError::PathTooLong;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Path too long"));
    }

    #[test]
    fn map_fs_error_filename_too_long() {
        let err = FSError::FilenameTooLong;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Filename too long"));
    }

    #[test]
    fn map_fs_error_too_many_open_files() {
        let err = FSError::TooManyOpenFiles;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::ServiceUnavailable);
        assert!(resp.message.contains("Too many open files"));
    }

    #[test]
    fn map_fs_error_cross_device_link() {
        let err = FSError::CrossDeviceLink;
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Cross device link"));
    }

    #[test]
    fn map_fs_error_invalid_utf8() {
        let err = FSError::InvalidUtf8 {
            message: "bad bytes".to_string(),
        };
        let resp = map_fs_error(err);
        assert_eq!(resp.code, ErrorCode::BadRequest);
        assert!(resp.message.contains("Invalid UTF-8"));
        assert!(resp.message.contains("bad bytes"));
    }
}
