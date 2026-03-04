//! Server configuration.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

use crate::auth::JwtConfig;

/// Validation error for server configuration.
#[derive(Debug, Clone)]
pub struct ConfigValidationError {
    pub message: String,
}

impl fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConfigValidationError {}

/// Server configuration loaded from TOML file.
#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    /// Server host address.
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Filesystem mount points.
    #[serde(default)]
    pub mounts: HashMap<String, MountConfig>,

    /// JWT authentication configuration.
    #[serde(default)]
    pub jwt: JwtConfig,

    /// Rate limit configuration (reloadable via SIGHUP).
    #[serde(default)]
    pub ratelimit: RatelimitConfigOption,

    /// Stat cache configuration (reloadable via SIGHUP).
    #[serde(default)]
    pub stat_cache: StatCacheConfigOption,
}

/// Optional rate limit config for TOML; missing uses defaults.
#[derive(Debug, Deserialize, Clone)]
pub struct RatelimitConfigOption {
    /// Max requests per window.
    #[serde(default = "default_max_requests")]
    pub max_requests: u32,
    /// Window duration in seconds.
    #[serde(default = "default_window_secs")]
    pub window_secs: u64,
}

fn default_max_requests() -> u32 {
    100
}

fn default_window_secs() -> u64 {
    60
}

impl Default for RatelimitConfigOption {
    fn default() -> Self {
        Self {
            max_requests: default_max_requests(),
            window_secs: default_window_secs(),
        }
    }
}

/// Optional stat cache config for TOML; missing uses defaults.
#[derive(Debug, Deserialize, Clone)]
pub struct StatCacheConfigOption {
    /// TTL for cached stat entries in seconds.
    #[serde(default = "default_stat_cache_ttl_secs")]
    pub ttl_secs: u64,
}

fn default_stat_cache_ttl_secs() -> u64 {
    2
}

impl Default for StatCacheConfigOption {
    fn default() -> Self {
        Self {
            ttl_secs: default_stat_cache_ttl_secs(),
        }
    }
}

/// Optional per-mount cache configuration.
/// Used by plugins that support caching (e.g. future Foyer-backed or content cache).
#[derive(Debug, Default, Deserialize, Clone)]
pub struct MountCacheConfig {
    /// Enable caching for this mount. Default false.
    #[serde(default)]
    pub enabled: bool,

    /// TTL for cached entries in seconds. Optional; plugin-specific default if not set.
    pub ttl_secs: Option<u64>,

    /// Max number of cached entries. Optional; plugin-specific default if not set.
    pub max_entries: Option<usize>,
}

/// Mount point configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct MountConfig {
    /// Mount path (e.g., "/memfs").
    pub path: String,

    /// Filesystem type ("memfs", "devfs", "localfs", "proxyfs", "kvfs", "streamfs", "vectorfs", "queuefs").
    #[serde(default = "default_fs_type")]
    pub fs_type: String,

    /// Base URL for proxyfs.
    pub url: Option<String>,

    /// Root path for localfs.
    pub root: Option<String>,

    /// Buffer size for streamfs (max messages per stream).
    #[serde(default = "default_buffer_size")]
    pub buffer_size: Option<usize>,

    /// History size for streamfs.
    #[serde(default = "default_history_size")]
    pub history_size: Option<usize>,

    /// Max concurrent streams for streamfs.
    #[serde(default = "default_max_streams")]
    pub max_streams: Option<usize>,

    /// Storage URI for vectorfs: local path (e.g. `./vector_db`) or S3 URI (`s3://bucket/prefix`).
    /// When unset, defaults to `./vector_db`.
    pub storage_uri: Option<String>,

    /// [Deprecated] Use `storage_uri` instead. If set and `storage_uri` is unset, used as storage path.
    pub db_path: Option<String>,

    /// SQLite path for kvfs persistence; when set, kvfs uses this file. When unset, kvfs is in-memory.
    pub database_path: Option<String>,

    /// Embedding API URL for vectorfs (optional). When set, document/query text is sent here for embedding.
    pub embedding_url: Option<String>,

    /// LanceDB table name for vectorfs. When unset, defaults to `"vectors"`. Use different names per mount for multi-tenant (e.g. per-agent tables).
    pub table_name: Option<String>,

    /// Vector dimension for vectorfs embeddings. Must match the embedding API output (e.g. 384 for sentence-transformers, 1536 for OpenAI text-embedding-3-small). When unset, defaults to 384.
    pub dimension: Option<usize>,

    /// Optional per-mount cache configuration (for plugins that support caching).
    #[serde(default)]
    pub cache: Option<MountCacheConfig>,

    /// Permissions for creating files/directories.
    #[serde(default = "default_permissions")]
    #[allow(dead_code)]
    pub perm: u32,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_fs_type() -> String {
    "memfs".to_string()
}

fn default_permissions() -> u32 {
    0o755
}

fn default_buffer_size() -> Option<usize> {
    Some(1000)
}

fn default_history_size() -> Option<usize> {
    Some(100)
}

fn default_max_streams() -> Option<usize> {
    Some(100)
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            mounts: HashMap::new(),
            jwt: JwtConfig::default(),
            ratelimit: RatelimitConfigOption::default(),
            stat_cache: StatCacheConfigOption::default(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from file or return default if file doesn't exist.
    pub fn from_file_or_default(path: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        match path {
            Some(p) => Self::from_file(p),
            None => Ok(Self::default()),
        }
    }

    /// Override host and port from environment variables if set.
    /// - `RACFS_HOST` – bind address (e.g. `0.0.0.0`)
    /// - `RACFS_PORT` – port number (e.g. `8080`)
    pub fn apply_env_overrides(&mut self) {
        if let Ok(s) = std::env::var("RACFS_HOST") {
            self.host = s;
        }
        if let Ok(s) = std::env::var("RACFS_PORT")
            && let Ok(p) = s.parse::<u16>()
        {
            self.port = p;
        }
    }

    /// Validate configuration. Returns an error listing all issues found.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        let mut errors = Vec::new();

        if self.host.trim().is_empty() {
            errors.push("host must be non-empty".to_string());
        }

        for (name, m) in &self.mounts {
            if m.path.trim().is_empty() {
                errors.push(format!("mounts.{}: path must be non-empty", name));
            } else if !m.path.starts_with('/') {
                errors.push(format!("mounts.{}: path must start with '/'", name));
            }

            match m.fs_type.as_str() {
                "localfs" => {
                    if m.root.as_ref().is_none_or(|r| r.trim().is_empty()) {
                        errors.push(format!("mounts.{}: localfs requires 'root' path", name));
                    }
                }
                "proxyfs" => {
                    if m.url.as_ref().is_none_or(|u| u.trim().is_empty()) {
                        errors.push(format!("mounts.{}: proxyfs requires 'url'", name));
                    }
                }
                "streamfs" => {
                    if let Some(s) = m.buffer_size
                        && s == 0
                    {
                        errors.push(format!("mounts.{}: streamfs buffer_size must be > 0", name));
                    }
                    if let Some(s) = m.history_size
                        && s == 0
                    {
                        errors.push(format!(
                            "mounts.{}: streamfs history_size must be > 0",
                            name
                        ));
                    }
                    if let Some(s) = m.max_streams
                        && s == 0
                    {
                        errors.push(format!("mounts.{}: streamfs max_streams must be > 0", name));
                    }
                }
                _ => {}
            }

            if let Some(ref cache) = m.cache
                && cache.enabled
            {
                if let Some(ttl) = cache.ttl_secs
                    && ttl == 0
                {
                    errors.push(format!(
                        "mounts.{}: cache.ttl_secs must be > 0 when cache is enabled",
                        name
                    ));
                }
                if let Some(max) = cache.max_entries
                    && max == 0
                {
                    errors.push(format!(
                        "mounts.{}: cache.max_entries must be > 0 when cache is enabled",
                        name
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigValidationError {
                message: errors.join("; "),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert!(config.mounts.is_empty());
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
            host = "0.0.0.0"
            port = 9000

            [mounts.memfs]
            path = "/memfs"
            fs_type = "memfs"

            [mounts.dev]
            path = "/dev"
            fs_type = "devfs"
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9000);
        assert_eq!(config.mounts.len(), 2);
    }

    #[test]
    fn test_apply_env_overrides() {
        unsafe {
            std::env::set_var("RACFS_HOST", "0.0.0.0");
            std::env::set_var("RACFS_PORT", "9000");
        }
        let mut config = ServerConfig::default();
        config.apply_env_overrides();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9000);

        // Invalid port is ignored
        unsafe {
            std::env::set_var("RACFS_PORT", "not_a_number");
        }
        let mut config2 = ServerConfig {
            port: 8080,
            ..Default::default()
        };
        config2.apply_env_overrides();
        assert_eq!(config2.port, 8080);

        unsafe {
            std::env::remove_var("RACFS_HOST");
            std::env::remove_var("RACFS_PORT");
        }
    }

    #[test]
    fn test_validate_ok() {
        assert!(ServerConfig::default().validate().is_ok());
    }

    #[test]
    fn test_validate_empty_host() {
        let config = ServerConfig {
            host: "".to_string(),
            ..Default::default()
        };
        let r = config.validate();
        assert!(r.is_err());
        assert!(r.unwrap_err().message.contains("host"));
    }

    #[test]
    fn test_validate_localfs_missing_root() {
        let toml_str = r#"
            [mounts.data]
            path = "/data"
            fs_type = "localfs"
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        let msg = r.unwrap_err().message;
        assert!(msg.contains("localfs"));
        assert!(msg.contains("root"));
    }

    #[test]
    fn test_validate_proxyfs_missing_url() {
        let toml_str = r#"
            [mounts.r]
            path = "/remote"
            fs_type = "proxyfs"
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        let msg = r.unwrap_err().message;
        assert!(msg.contains("proxyfs"));
        assert!(msg.contains("url"));
    }

    #[test]
    fn test_validate_path_must_start_with_slash() {
        let toml_str = r#"
            [mounts.m]
            path = "memfs"
            fs_type = "memfs"
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        assert!(r.unwrap_err().message.contains("start with"));
    }

    #[test]
    fn test_ratelimit_and_stat_cache_config() {
        let toml_str = r#"
            [ratelimit]
            max_requests = 200
            window_secs = 120

            [stat_cache]
            ttl_secs = 5
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.ratelimit.max_requests, 200);
        assert_eq!(config.ratelimit.window_secs, 120);
        assert_eq!(config.stat_cache.ttl_secs, 5);
    }

    #[test]
    fn test_mount_cache_config() {
        let toml_str = r#"
            [mounts.memfs]
            path = "/memfs"
            fs_type = "memfs"

            [mounts.cached]
            path = "/cached"
            fs_type = "memfs"
            [mounts.cached.cache]
            enabled = true
            ttl_secs = 60
            max_entries = 1000
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(config.mounts.get("memfs").unwrap().cache.is_none());
        let cached = config.mounts.get("cached").unwrap();
        let cache = cached.cache.as_ref().unwrap();
        assert!(cache.enabled);
        assert_eq!(cache.ttl_secs, Some(60));
        assert_eq!(cache.max_entries, Some(1000));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_cache_ttl_zero() {
        let toml_str = r#"
            [mounts.x]
            path = "/x"
            fs_type = "memfs"
            [mounts.x.cache]
            enabled = true
            ttl_secs = 0
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        let msg = r.unwrap_err().message;
        assert!(msg.contains("cache"));
        assert!(msg.contains("ttl_secs"));
    }

    #[test]
    fn test_validate_streamfs_zero_buffer() {
        let toml_str = r#"
            [mounts.s]
            path = "/streams"
            fs_type = "streamfs"
            buffer_size = 0
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        let msg = r.unwrap_err().message;
        assert!(msg.contains("streamfs"));
        assert!(msg.contains("buffer_size"));
    }

    #[test]
    fn test_validate_streamfs_zero_history_and_max_streams() {
        let toml_str = r#"
            [mounts.s]
            path = "/streams"
            fs_type = "streamfs"
            history_size = 0
            max_streams = 0
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        let msg = r.unwrap_err().message;
        assert!(msg.contains("streamfs"));
    }

    #[test]
    fn test_validate_mount_empty_path() {
        let toml_str = r#"
            [mounts.bad]
            path = ""
            fs_type = "memfs"
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        assert!(r.unwrap_err().message.contains("path"));
    }

    #[test]
    fn test_validate_cache_max_entries_zero() {
        let toml_str = r#"
            [mounts.x]
            path = "/x"
            fs_type = "memfs"
            [mounts.x.cache]
            enabled = true
            ttl_secs = 60
            max_entries = 0
        "#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = config.validate();
        assert!(r.is_err());
        let msg = r.unwrap_err().message;
        assert!(msg.contains("max_entries"));
    }
}
