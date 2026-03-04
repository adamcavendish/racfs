//! Input validation for API request parameters.
//!
//! Validates paths, mode bits, and sizes before passing to the VFS to fail fast
//! with 400 Bad Request for invalid input.

use racfs_http_error::{ErrorCode, HttpErrorResponse};

/// Maximum allowed path length (bytes).
const MAX_PATH_LEN: usize = 8192;

/// Validates an API path: absolute, no null bytes, no ".." traversal, length limit.
pub fn validate_path(path: &str) -> Result<(), HttpErrorResponse> {
    if path.is_empty() {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "path must be non-empty",
            "path: (empty)",
        ));
    }
    if !path.starts_with('/') {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "path must be absolute (start with /)",
            format!("path: {}", path),
        ));
    }
    if path.contains('\0') {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "path must not contain null bytes",
            "path: (contains NUL)",
        ));
    }
    if path.contains("..") {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "path must not contain ..",
            format!("path: {}", path),
        ));
    }
    if path.len() > MAX_PATH_LEN {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "path too long",
            format!("path length {} exceeds max {}", path.len(), MAX_PATH_LEN),
        ));
    }
    Ok(())
}

/// Validates chmod mode: Unix permission bits only (0o7777 = 12 bits).
pub fn validate_mode(mode: u32) -> Result<(), HttpErrorResponse> {
    if mode > 0o7777 {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "mode must be a valid Unix permission (0o0000..0o7777)",
            format!("mode: 0o{:o}", mode),
        ));
    }
    Ok(())
}

/// Validates truncate size (no-op check; backend may enforce its own limits).
pub fn validate_truncate_size(_size: u64) -> Result<(), HttpErrorResponse> {
    Ok(())
}

/// Maximum allowed write body size (bytes) to prevent DoS.
const MAX_WRITE_BODY_LEN: usize = 16 * 1024 * 1024; // 16 MiB

/// Validates write/request body length.
pub fn validate_write_data_len(len: usize) -> Result<(), HttpErrorResponse> {
    if len > MAX_WRITE_BODY_LEN {
        return Err(HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "request body too large",
            format!("size {} exceeds max {} bytes", len, MAX_WRITE_BODY_LEN),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_empty() {
        assert!(validate_path("").is_err());
    }

    #[test]
    fn test_validate_path_relative() {
        assert!(validate_path("memfs/foo").is_err());
        assert!(validate_path("foo").is_err());
    }

    #[test]
    fn test_validate_path_absolute_ok() {
        assert!(validate_path("/").is_ok());
        assert!(validate_path("/memfs").is_ok());
        assert!(validate_path("/memfs/foo").is_ok());
    }

    #[test]
    fn test_validate_path_null_byte() {
        assert!(validate_path("/memfs/foo\0bar").is_err());
    }

    #[test]
    fn test_validate_path_dotdot() {
        assert!(validate_path("/memfs/../etc/passwd").is_err());
        assert!(validate_path("/memfs/..").is_err());
        assert!(validate_path("/..").is_err());
    }

    #[test]
    fn test_validate_path_too_long() {
        let long = "/".to_string() + &"a".repeat(MAX_PATH_LEN);
        assert!(validate_path(&long).is_err());
    }

    #[test]
    fn test_validate_mode_ok() {
        assert!(validate_mode(0).is_ok());
        assert!(validate_mode(0o644).is_ok());
        assert!(validate_mode(0o7777).is_ok());
    }

    #[test]
    fn test_validate_mode_invalid() {
        assert!(validate_mode(0o10000).is_err());
        assert!(validate_mode(u32::MAX).is_err());
    }

    #[test]
    fn test_validate_write_data_len_ok() {
        assert!(validate_write_data_len(0).is_ok());
        assert!(validate_write_data_len(1024).is_ok());
        assert!(validate_write_data_len(MAX_WRITE_BODY_LEN).is_ok());
    }

    #[test]
    fn test_validate_write_data_len_too_large() {
        assert!(validate_write_data_len(MAX_WRITE_BODY_LEN + 1).is_err());
    }

    #[test]
    fn test_validate_truncate_size_ok() {
        assert!(validate_truncate_size(0).is_ok());
        assert!(validate_truncate_size(1024).is_ok());
        assert!(validate_truncate_size(u64::MAX).is_ok());
    }
}
