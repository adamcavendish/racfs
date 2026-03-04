//! HTTP client filesystem (httpfs) plugin.
//!
//! Provides a POSIX-like interface for making HTTP requests through
//! a virtual filesystem. Users write request data to special files
//! and read responses from other files.
//!
//! # Structure
//!
//! ```text
//! /
//! ├── requests/              # Pending requests (write here)
//! │   └── {request_id}/
//! │       ├── url           # Target URL (write)
//! │       ├── method        # GET/POST/PUT/DELETE (write)
//! │       ├── headers.json  # Request headers (write, optional)
//! │       ├── body          # Request body (write, optional)
//! │       └── trigger       # Write "send" to execute request
//! ├── responses/            # Completed responses (read here)
//! │   └── {request_id}/
//! │       ├── status        # HTTP status code
//! │       ├── headers.json  # Response headers
//! │       └── body          # Response body
//! └── cache/                # Cached GET responses (optional)
//!     └── {url_hash}/
//!         └── body
//! ```

mod fs;

pub use fs::HttpFS;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;
    use crate::fs::{parse_cache_control, parse_request_id, url_to_cache_key};

    #[tokio::test]
    async fn test_create_request_directory() {
        let fs = HttpFS::new();
        fs.mkdir(&PathBuf::from("/requests/test001"), 0o755)
            .await
            .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/requests")).await.unwrap();
        assert!(
            entries
                .iter()
                .any(|e| e.path.as_path() == std::path::Path::new("/requests/test001"))
        );
    }

    #[tokio::test]
    async fn test_write_url() {
        let fs = HttpFS::new();
        fs.mkdir(&PathBuf::from("/requests/test001"), 0o755)
            .await
            .unwrap();
        fs.create(&PathBuf::from("/requests/test001/url"))
            .await
            .unwrap();

        fs.write(
            &PathBuf::from("/requests/test001/url"),
            b"https://example.com",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let read = fs
            .read(&PathBuf::from("/requests/test001/url"), 0, -1)
            .await
            .unwrap();
        assert_eq!(read, b"https://example.com");
    }

    #[tokio::test]
    async fn test_write_method() {
        let fs = HttpFS::new();
        fs.mkdir(&PathBuf::from("/requests/test001"), 0o755)
            .await
            .unwrap();
        fs.create(&PathBuf::from("/requests/test001/method"))
            .await
            .unwrap();

        fs.write(
            &PathBuf::from("/requests/test001/method"),
            b"GET",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let read = fs
            .read(&PathBuf::from("/requests/test001/method"), 0, -1)
            .await
            .unwrap();
        assert_eq!(read, b"GET");
    }

    #[tokio::test]
    async fn test_parse_request_id() {
        let id = parse_request_id(&PathBuf::from("/requests/req001/url")).unwrap();
        assert_eq!(id, "req001");
    }

    #[tokio::test]
    async fn test_parse_request_id_invalid() {
        assert!(parse_request_id(&PathBuf::from("/invalid/path")).is_err());
    }

    #[tokio::test]
    async fn test_responses_read_only() {
        let fs = HttpFS::new();
        fs.mkdir(&PathBuf::from("/responses/test001"), 0o755)
            .await
            .unwrap();
        fs.create(&PathBuf::from("/responses/test001/status"))
            .await
            .unwrap();

        let result = fs
            .write(
                &PathBuf::from("/responses/test001/status"),
                b"test",
                0,
                WriteFlags::none(),
            )
            .await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_cannot_create_file_in_requests_root() {
        let fs = HttpFS::new();
        assert!(
            fs.create(&PathBuf::from("/requests/direct_file.txt"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_stat_root() {
        let fs = HttpFS::new();
        let meta = fs.stat(&PathBuf::from("/")).await.unwrap();
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_read_dir_root() {
        let fs = HttpFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/requests")));
        assert!(paths.contains(&PathBuf::from("/responses")));
    }

    #[test]
    fn test_url_to_cache_key() {
        let a = url_to_cache_key("https://example.com/foo");
        let b = url_to_cache_key("https://example.com/foo");
        let c = url_to_cache_key("https://example.com/bar");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_parse_cache_control() {
        let mut h = HashMap::new();
        h.insert("cache-control".to_string(), "max-age=300".to_string());
        let d = parse_cache_control(&h);
        assert_eq!(d.max_age_secs, Some(300));
        assert!(!d.no_store);

        h.clear();
        h.insert("Cache-Control".to_string(), "no-store".to_string());
        let d = parse_cache_control(&h);
        assert!(d.no_store);

        h.clear();
        h.insert(
            "cache-control".to_string(),
            "no-store, max-age=0".to_string(),
        );
        let d = parse_cache_control(&h);
        assert!(d.no_store);
    }

    #[tokio::test]
    async fn test_get_cached_response_miss() {
        let fs = HttpFS::new();
        let key = url_to_cache_key("https://example.com/");
        assert!(fs.get_cached_response(&key).is_none());
    }

    #[tokio::test]
    async fn test_set_and_get_cached_response() {
        let fs = HttpFS::new();
        let key = url_to_cache_key("https://example.com/foo");
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "text/plain".to_string());
        fs.set_cached_response(&key, 200, &headers, b"cached body".to_vec(), 60)
            .unwrap();

        let cached = fs.get_cached_response(&key).unwrap();
        assert_eq!(cached.0, 200);
        assert_eq!(
            cached.1.get("content-type").map(|s| s.as_str()),
            Some("text/plain")
        );
        assert_eq!(cached.2, b"cached body");
    }

    #[tokio::test]
    async fn test_cached_response_expired() {
        let fs = HttpFS::new();
        let key = url_to_cache_key("https://example.com/expired");
        let headers = HashMap::new();
        fs.set_cached_response(&key, 200, &headers, b"old".to_vec(), 1)
            .unwrap();

        assert!(fs.get_cached_response(&key).is_some());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        assert!(fs.get_cached_response(&key).is_none());
    }
}
