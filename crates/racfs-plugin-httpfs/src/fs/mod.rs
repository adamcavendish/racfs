//! HTTP client filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::HashMap,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use parking_lot::RwLock;
use racfs_core::{
    error::FSError,
    filesystem::{FileSystem, WriteFS},
    metadata::FileMetadata,
};

/// HTTP client filesystem.
///
/// Manages HTTP requests and responses in memory.
pub struct HttpFS {
    /// Storage for all files and directories
    pub(super) files: Arc<RwLock<HashMap<PathBuf, FileEntry>>>,
}

#[derive(Clone)]
pub(super) struct FileEntry {
    pub(super) data: Vec<u8>,
    pub(super) metadata: FileMetadata,
}

/// Parse a request ID from a path like "/requests/req001/url"
pub(crate) fn parse_request_id(path: &Path) -> Result<String, FSError> {
    let components: Vec<_> = path.components().collect();

    if components.len() < 3
        || components[0] != std::path::Component::RootDir
        || components[1] != std::path::Component::Normal(std::ffi::OsStr::new("requests"))
    {
        return Err(FSError::InvalidInput {
            message: "invalid request path".to_string(),
        });
    }

    if let Some(std::path::Component::Normal(name)) = components.get(2) {
        name.to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| FSError::InvalidInput {
                message: "invalid request ID".to_string(),
            })
    } else {
        Err(FSError::InvalidInput {
            message: "invalid request ID".to_string(),
        })
    }
}

/// Compute a filesystem-safe cache key from a URL.
pub(crate) fn url_to_cache_key(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Cache directive from Cache-Control header.
#[derive(Clone, Debug)]
pub(crate) struct CacheDirective {
    /// Max age in seconds; None if not specified.
    pub max_age_secs: Option<u64>,
    /// True if no-store was present.
    pub no_store: bool,
}

/// Parse Cache-Control header into CacheDirective.
pub(crate) fn parse_cache_control(headers: &HashMap<String, String>) -> CacheDirective {
    let value = headers
        .get("cache-control")
        .or_else(|| headers.get("Cache-Control"))
        .map(|s| s.as_str())
        .unwrap_or("");
    let mut max_age_secs: Option<u64> = None;
    let mut no_store = false;
    for part in value.split(',') {
        let part = part.trim();
        if part.eq_ignore_ascii_case("no-store") {
            no_store = true;
        } else if part.eq_ignore_ascii_case("no-cache") {
            max_age_secs = Some(0);
        } else if part.to_lowercase().starts_with("max-age=")
            && let Ok(n) = part[8..].trim().parse::<u64>()
        {
            max_age_secs = Some(n);
        }
    }
    CacheDirective {
        max_age_secs,
        no_store,
    }
}

/// Default cache TTL when server sends no max-age (seconds).
const DEFAULT_CACHE_TTL_SECS: u64 = 60;

impl HttpFS {
    /// Create a new HTTP filesystem.
    pub fn new() -> Self {
        let fs = Self {
            files: Arc::new(RwLock::new(HashMap::new())),
        };

        fs.init_directories();

        fs
    }

    /// Initialize the top-level directories.
    fn init_directories(&self) {
        let mut files = self.files.write();

        files.insert(
            PathBuf::from("/"),
            FileEntry {
                data: Vec::new(),
                metadata: FileMetadata::directory(PathBuf::from("/")),
            },
        );

        files.insert(
            PathBuf::from("/requests"),
            FileEntry {
                data: Vec::new(),
                metadata: FileMetadata::directory(PathBuf::from("/requests")),
            },
        );

        files.insert(
            PathBuf::from("/responses"),
            FileEntry {
                data: Vec::new(),
                metadata: FileMetadata::directory(PathBuf::from("/responses")),
            },
        );

        files.insert(
            PathBuf::from("/cache"),
            FileEntry {
                data: Vec::new(),
                metadata: FileMetadata::directory(PathBuf::from("/cache")),
            },
        );
    }

    pub(super) fn get_entry(&self, path: &Path) -> Result<FileEntry, FSError> {
        let files = self.files.read();
        files.get(path).cloned().ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })
    }

    pub(super) fn ensure_parent_exists(&self, path: &Path) -> Result<(), FSError> {
        if let Some(parent) = path.parent()
            && parent.as_os_str() != "/"
        {
            let files = self.files.read();
            let parent_entry = files.get(parent).ok_or_else(|| FSError::NotFound {
                path: parent.to_path_buf(),
            })?;

            if !parent_entry.metadata.is_directory() {
                return Err(FSError::NotADirectory {
                    path: parent.to_path_buf(),
                });
            }
        }
        Ok(())
    }

    /// Execute an HTTP request and store the response.
    fn execute_request_sync(&self, request_id: &str) -> Result<(), FSError> {
        let url_path = PathBuf::from(format!("/requests/{}/url", request_id));
        let method_path = PathBuf::from(format!("/requests/{}/method", request_id));
        let headers_path = PathBuf::from(format!("/requests/{}/headers.json", request_id));
        let body_path = PathBuf::from(format!("/requests/{}/body", request_id));

        let url = self.read_to_string_sync(&url_path)?;
        let method = self.read_to_string_sync(&method_path)?;
        let headers_json = self.read_to_string_sync(&headers_path).unwrap_or_default();
        let body = self.read_bytes_sync(&body_path).unwrap_or_default();

        let method_upper = method.trim().to_uppercase();
        let is_get = method_upper == "GET";
        let cache_key = if is_get {
            url_to_cache_key(url.trim())
        } else {
            String::new()
        };

        if is_get
            && let Some((status_code, headers, cached_body)) = self.get_cached_response(&cache_key)
        {
            self.store_response_direct(request_id, status_code, headers, cached_body)?;
            tracing::debug!(
                request_id = %request_id,
                url = %url,
                status = status_code,
                "served from cache"
            );
            return Ok(());
        }

        let headers: HashMap<String, String> = if headers_json.is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(&headers_json).map_err(|e| FSError::InvalidInput {
                message: format!("invalid headers.json: {}", e),
            })?
        };

        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| FSError::Io {
                message: format!("failed to build HTTP client: {}", e),
            })?;

        let mut request = match method_upper.as_str() {
            "GET" => client.get(url.trim()),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            "PATCH" => client.patch(&url),
            "HEAD" => client.head(&url),
            _ => {
                return Err(FSError::InvalidInput {
                    message: format!("unsupported HTTP method: {}", method),
                });
            }
        };

        for (key, value) in &headers {
            request = request.header(key, value);
        }

        if is_get && let Some(etag) = self.get_cached_etag(&cache_key) {
            request = request.header("If-None-Match", etag.trim_matches('"'));
        }

        let response =
            if !body.is_empty() && matches!(method_upper.as_str(), "POST" | "PUT" | "PATCH") {
                request.body(body).send().map_err(|e| FSError::Io {
                    message: format!("HTTP request failed: {}", e),
                })?
            } else {
                request.send().map_err(|e| FSError::Io {
                    message: format!("HTTP request failed: {}", e),
                })?
            };

        let status_code = response.status().as_u16();
        let response_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.to_string(), v.to_string()))
            })
            .collect();

        let response_body = if status_code == 304 && is_get {
            self.get_cached_body_only(&cache_key).unwrap_or_default()
        } else {
            response
                .bytes()
                .map_err(|e| FSError::Io {
                    message: format!("failed to read response body: {}", e),
                })?
                .to_vec()
        };

        self.store_response_direct(
            request_id,
            status_code,
            response_headers.clone(),
            response_body.clone(),
        )?;

        if is_get && status_code == 200 {
            let directive = parse_cache_control(&response_headers);
            if !directive.no_store {
                let ttl = directive.max_age_secs.unwrap_or(DEFAULT_CACHE_TTL_SECS);
                if let Err(e) = self.set_cached_response(
                    &cache_key,
                    status_code,
                    &response_headers,
                    response_body,
                    ttl,
                ) {
                    tracing::warn!(error = %e, "failed to write cache");
                }
            }
        }

        tracing::debug!(
            request_id = %request_id,
            url = %url,
            status = status_code,
            "HTTP request executed"
        );

        Ok(())
    }

    fn store_response_direct(
        &self,
        request_id: &str,
        status_code: u16,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(), FSError> {
        let mut files = self.files.write();

        let response_dir = PathBuf::from(format!("/responses/{}", request_id));
        if !files.contains_key(&response_dir) {
            files.insert(
                response_dir.clone(),
                FileEntry {
                    data: Vec::new(),
                    metadata: FileMetadata::directory(response_dir.clone()),
                },
            );
        }

        let status_path = PathBuf::from(format!("/responses/{}/status", request_id));
        let status_data = status_code.to_string().into_bytes();
        let status_len = status_data.len();
        files.insert(
            status_path.clone(),
            FileEntry {
                data: status_data,
                metadata: FileMetadata::file(status_path, status_len as u64),
            },
        );

        let headers_path = PathBuf::from(format!("/responses/{}/headers.json", request_id));
        let headers_json = serde_json::to_string(&headers).map_err(|e| FSError::Io {
            message: format!("failed to serialize headers: {}", e),
        })?;
        let headers_data = headers_json.into_bytes();
        let headers_len = headers_data.len();
        files.insert(
            headers_path.clone(),
            FileEntry {
                data: headers_data,
                metadata: FileMetadata::file(headers_path, headers_len as u64),
            },
        );

        let body_path = PathBuf::from(format!("/responses/{}/body", request_id));
        let body_len = body.len();
        files.insert(
            body_path.clone(),
            FileEntry {
                data: body,
                metadata: FileMetadata::file(body_path, body_len as u64),
            },
        );

        Ok(())
    }

    fn read_to_string_sync(&self, path: &Path) -> Result<String, FSError> {
        let data = self.read_bytes_sync(path)?;
        String::from_utf8(data).map_err(|e| FSError::InvalidUtf8 {
            message: e.to_string(),
        })
    }

    fn read_bytes_sync(&self, path: &Path) -> Result<Vec<u8>, FSError> {
        let entry = self.get_entry(path)?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        Ok(entry.data)
    }

    /// Load cached GET response if present and not expired. Returns (status, headers, body).
    pub(crate) fn get_cached_response(
        &self,
        cache_key: &str,
    ) -> Option<(u16, HashMap<String, String>, Vec<u8>)> {
        let base = PathBuf::from(format!("/cache/{}", cache_key));
        let expiry_path = base.join("expiry");
        let expiry_str = self.read_to_string_sync(&expiry_path).ok()?;
        let expiry_secs: u64 = expiry_str.trim().parse().ok()?;
        let now = chrono::Utc::now().timestamp();
        if now > expiry_secs as i64 {
            return None;
        }
        let status_path = base.join("status");
        let status_str = self.read_to_string_sync(&status_path).ok()?;
        let status_code: u16 = status_str.trim().parse().ok()?;
        let headers_path = base.join("headers.json");
        let headers_json = self.read_to_string_sync(&headers_path).ok()?;
        let headers: HashMap<String, String> = serde_json::from_str(&headers_json).ok()?;
        let body = self.read_bytes_sync(&base.join("body")).ok()?;
        Some((status_code, headers, body))
    }

    /// Save GET response to cache.
    pub(crate) fn set_cached_response(
        &self,
        cache_key: &str,
        status_code: u16,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
        max_age_secs: u64,
    ) -> Result<(), FSError> {
        let base = PathBuf::from(format!("/cache/{}", cache_key));
        let mut files = self.files.write();

        if !files.contains_key(&base) {
            files.insert(
                base.clone(),
                FileEntry {
                    data: Vec::new(),
                    metadata: FileMetadata::directory(base.clone()),
                },
            );
        }

        let expiry_secs = chrono::Utc::now().timestamp() as u64 + max_age_secs;

        let status_path = base.join("status");
        files.insert(
            status_path.clone(),
            FileEntry {
                data: status_code.to_string().into_bytes(),
                metadata: FileMetadata::file(status_path, 3),
            },
        );

        let headers_path = base.join("headers.json");
        let headers_json = serde_json::to_string(headers).map_err(|e| FSError::Io {
            message: format!("serialize headers: {}", e),
        })?;
        let headers_data = headers_json.into_bytes();
        files.insert(
            headers_path.clone(),
            FileEntry {
                data: headers_data.clone(),
                metadata: FileMetadata::file(headers_path, headers_data.len() as u64),
            },
        );

        let body_path = base.join("body");
        let body_len = body.len();
        files.insert(
            body_path.clone(),
            FileEntry {
                data: body,
                metadata: FileMetadata::file(body_path, body_len as u64),
            },
        );

        let expiry_path = base.join("expiry");
        let expiry_data = expiry_secs.to_string().into_bytes();
        files.insert(
            expiry_path.clone(),
            FileEntry {
                data: expiry_data,
                metadata: FileMetadata::file(expiry_path, 20),
            },
        );

        if let Some(etag) = headers.get("etag").or_else(|| headers.get("ETag")) {
            let etag_path = base.join("etag");
            let etag_data = etag.as_bytes().to_vec();
            files.insert(
                etag_path.clone(),
                FileEntry {
                    data: etag_data,
                    metadata: FileMetadata::file(etag_path, etag.len() as u64),
                },
            );
        }

        Ok(())
    }

    fn get_cached_etag(&self, cache_key: &str) -> Option<String> {
        let etag_path = PathBuf::from(format!("/cache/{}/etag", cache_key));
        self.read_to_string_sync(&etag_path).ok()
    }

    fn get_cached_body_only(&self, cache_key: &str) -> Option<Vec<u8>> {
        let body_path = PathBuf::from(format!("/cache/{}/body", cache_key));
        self.read_bytes_sync(&body_path).ok()
    }
}

impl Default for HttpFS {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for HttpFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        let mut files = self.files.write();

        let entry = files.get_mut(path).ok_or_else(|| FSError::NotFound {
            path: path.to_path_buf(),
        })?;

        if entry.metadata.is_directory() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let current_size = entry.data.len() as u64;
        if size < current_size {
            entry.data.truncate(size as usize);
        } else if size > current_size {
            entry.data.resize(size as usize, 0);
        }

        entry.metadata.size = size;
        entry.metadata.modified = Some(chrono::Utc::now());

        tracing::debug!(path = %path.display(), size = size, "truncated");
        Ok(())
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        {
            let mut files = self.files.write();

            if let Some(entry) = files.get_mut(path) {
                entry.metadata.accessed = Some(chrono::Utc::now());
                entry.metadata.modified = Some(chrono::Utc::now());
                return Ok(());
            }
        }

        self.create(path).await?;

        {
            let mut files = self.files.write();
            if let Some(entry) = files.get_mut(path) {
                entry.metadata.accessed = Some(chrono::Utc::now());
                entry.metadata.modified = Some(chrono::Utc::now());
            }
        }

        tracing::debug!(path = %path.display(), "touched");
        Ok(())
    }
}
