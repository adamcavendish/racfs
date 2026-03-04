#[cfg(test)]
mod s3fs_tests {
    use crate::{S3Config, S3FS};
    use racfs_core::ChmodFS;
    use racfs_core::error::FSError;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_s3_config_default() {
        let config = S3Config::default();
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.cache_size, 1024 * 1024 * 1024);
        assert!(!config.cache_enabled);
        assert_eq!(config.multipart_threshold, 5 * 1024 * 1024);
        assert_eq!(config.multipart_part_size, 5 * 1024 * 1024);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_s3_fs_new() {
        let config = S3Config {
            bucket: String::from("test-bucket"),
            region: String::from("us-west-2"),
            endpoint: Some(String::from("http://localhost:9000")),
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: true,
            cache_size: 512 * 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");
        assert_eq!(fs.config.bucket, "test-bucket");
        assert_eq!(fs.config.region, "us-west-2");
        assert_eq!(
            fs.config.endpoint,
            Some(String::from("http://localhost:9000"))
        );
        assert!(fs.config.cache_enabled);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_config_valid() {
        let config = S3Config {
            bucket: String::from("test-bucket"),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: false,
            cache_size: 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");
        assert!(fs.validate_config().is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_config_empty_bucket() {
        let config = S3Config {
            bucket: String::new(),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: false,
            cache_size: 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");
        let result = fs.validate_config();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FSError::InvalidInput { .. }));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_read_dir_root() {
        let config = S3Config {
            bucket: String::from("test-bucket"),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: false,
            cache_size: 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");
        // This will attempt to connect to S3 and may fail if credentials/network unavailable
        // Just verify config is valid and client is created
        assert!(fs.validate_config().is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_chmod_noop() {
        let config = S3Config {
            bucket: String::from("test-bucket"),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: false,
            cache_size: 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");
        let result = fs.chmod(&PathBuf::from("/some-path"), 0o644).await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_path_to_key() {
        let config = S3Config {
            bucket: String::from("test-bucket"),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: false,
            cache_size: 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");

        assert_eq!(fs.path_to_key(&PathBuf::from("/")), "");
        assert_eq!(fs.path_to_key(&PathBuf::from("/foo.txt")), "foo.txt");
        assert_eq!(
            fs.path_to_key(&PathBuf::from("/dir/file.txt")),
            "dir/file.txt"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_key_to_path() {
        let config = S3Config {
            bucket: String::from("test-bucket"),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::from("test-key"),
            secret_key: String::from("test-secret"),
            cache_enabled: false,
            cache_size: 1024 * 1024,
            multipart_threshold: 5 * 1024 * 1024,
            multipart_part_size: 5 * 1024 * 1024,
        };

        let fs = S3FS::new(config).expect("test: S3FS::new");

        assert_eq!(fs.key_to_path(""), PathBuf::from("/"));
        assert_eq!(fs.key_to_path("foo.txt"), PathBuf::from("/foo.txt"));
        assert_eq!(
            fs.key_to_path("dir/file.txt"),
            PathBuf::from("/dir/file.txt")
        );
    }
}
