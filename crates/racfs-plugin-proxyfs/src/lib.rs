//! Proxy filesystem (proxyfs) - forwards to remote RACFS server.

mod fs;

pub use fs::ProxyFS;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{ChmodRequest, CreateDirectoryRequest, FileQuery, RenameRequest, WriteRequest};

    #[tokio::test]
    async fn test_new() {
        let fs = ProxyFS::new("http://localhost:8080".to_string());
        assert_eq!(fs.base_url, "http://localhost:8080");
    }

    #[test]
    fn test_file_query_serialization() {
        let query = FileQuery {
            path: "/test/file.txt".to_string(),
        };
        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("/test/file.txt"));
    }

    #[test]
    fn test_write_request_serialization() {
        let request = WriteRequest {
            path: "/test.txt".to_string(),
            data: "Hello".to_string(),
            offset: Some(10),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("/test.txt"));
        assert!(json.contains("Hello"));
        assert!(json.contains("10"));
    }

    #[test]
    fn test_rename_request_serialization() {
        let request = RenameRequest {
            old_path: "/old.txt".to_string(),
            new_path: "/new.txt".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("/old.txt"));
        assert!(json.contains("/new.txt"));
    }

    #[test]
    fn test_chmod_request_serialization() {
        let request = ChmodRequest {
            path: "/test.txt".to_string(),
            mode: 0o644,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("/test.txt"));
        assert!(json.contains("420"));
    }

    #[test]
    fn test_create_directory_request_serialization() {
        let request = CreateDirectoryRequest {
            path: "/mydir".to_string(),
            perm: Some(0o755),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("/mydir"));
        assert!(json.contains("493"));
    }

    #[tokio::test]
    async fn test_client_creation() {
        let fs = ProxyFS::new("http://example.com".to_string());
        let _ = &fs.client;
    }

    #[test]
    fn test_base_url_trailing_slash() {
        let fs1 = ProxyFS::new("http://localhost:8080/".to_string());
        let fs2 = ProxyFS::new("http://localhost:8080".to_string());

        assert!(fs1.base_url.ends_with('/'));
        assert!(!fs2.base_url.ends_with('/'));
    }
}
