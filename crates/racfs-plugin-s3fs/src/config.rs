//! Configuration for the S3 filesystem plugin.

/// Configuration for S3 filesystem.
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region.
    pub region: String,
    /// Custom endpoint (for MinIO or other S3-compatible services).
    pub endpoint: Option<String>,
    /// AWS access key ID.
    pub access_key: String,
    /// AWS secret access key.
    pub secret_key: String,
    /// Enable local caching.
    pub cache_enabled: bool,
    /// Maximum cache size in bytes.
    pub cache_size: usize,
    /// Minimum object size (bytes) to use multipart upload. Default 5 MiB (S3 minimum part size).
    pub multipart_threshold: usize,
    /// Part size for multipart uploads (bytes). Default 5 MiB (S3 minimum).
    pub multipart_part_size: usize,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket: String::new(),
            region: String::from("us-east-1"),
            endpoint: None,
            access_key: String::new(),
            secret_key: String::new(),
            cache_enabled: false,
            cache_size: 1024 * 1024 * 1024,       // 1GB default
            multipart_threshold: 5 * 1024 * 1024, // 5 MiB
            multipart_part_size: 5 * 1024 * 1024, // 5 MiB (S3 minimum)
        }
    }
}
