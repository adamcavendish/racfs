//! Error types for RACFS FUSE

use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Failed to mount at {}: {}", path, message))]
    MountFailed { path: String, message: String },

    #[snafu(display("Client error: {}", source))]
    Client { source: racfs_client::Error },

    #[snafu(display("Invalid path: {}", path))]
    InvalidPath { path: String },

    #[snafu(display("Inode not found: {}", inode))]
    InodeNotFound { inode: u64 },

    #[snafu(display("IO error: {}", source))]
    Io { source: std::io::Error },
}

impl From<racfs_client::Error> for Error {
    fn from(source: racfs_client::Error) -> Self {
        Error::Client { source }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io { source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_mount_failed_display() {
        let e = Error::MountFailed {
            path: "/tmp/mnt".to_string(),
            message: "permission denied".to_string(),
        };
        let s = e.to_string();
        assert!(s.contains("/tmp/mnt"));
        assert!(s.contains("permission denied"));
    }

    #[test]
    fn error_invalid_path_display() {
        let e = Error::InvalidPath {
            path: "/bad/path".to_string(),
        };
        assert!(e.to_string().contains("Invalid path"));
        assert!(e.to_string().contains("/bad/path"));
    }

    #[test]
    fn error_inode_not_found_display() {
        let e = Error::InodeNotFound { inode: 42 };
        assert!(e.to_string().contains("42"));
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::other("io err");
        let e: Error = io_err.into();
        assert!(matches!(e, Error::Io { .. }));
        assert!(e.to_string().contains("io err"));
    }

    #[test]
    fn error_from_client() {
        let client_err = racfs_client::Error::Api {
            code: "NotFound".to_string(),
            message: "not found".to_string(),
            detail: None,
        };
        let e: Error = client_err.into();
        assert!(matches!(e, Error::Client { .. }));
    }
}
