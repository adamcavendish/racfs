//! RACFS HTTP Client

use parking_lot::RwLock;
use reqwest::Client as ReqwestClient;
use snafu::Snafu;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::types::*;

/// Errors returned by the RACFS client.
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("HTTP error: {source}"))]
    Http { source: reqwest::Error },

    #[snafu(display("API error ({code}): {message}{}", detail.as_ref().map(|d| format!(" ({})", d)).unwrap_or_default()))]
    Api {
        code: String,
        message: String,
        detail: Option<String>,
    },

    #[snafu(display("Circuit breaker open: backend unavailable until cooldown elapses"))]
    CircuitOpen,

    #[snafu(display("Not authenticated"))]
    NotAuthenticated,

    #[snafu(display("Internal client error: {message}"))]
    Internal { message: String },
}

/// Default number of retries for transient network failures.
const DEFAULT_MAX_RETRIES: u32 = 3;
/// Initial backoff duration before first retry.
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 100;
/// Default circuit breaker: open after this many consecutive failures.
const DEFAULT_CIRCUIT_FAILURE_THRESHOLD: u32 = 5;
/// Default circuit breaker: stay open for this duration before half-open.
const DEFAULT_CIRCUIT_OPEN_DURATION_SECS: u64 = 30;

/// Auth state for the client (token storage).
#[derive(Clone, Default)]
pub struct AuthState {
    pub token: Option<String>,
}

/// Circuit breaker state: stops calling the backend after repeated failures.
#[derive(Debug, Default)]
struct CircuitState {
    failure_count: u32,
    open_until: Option<Instant>,
}

/// RACFS HTTP client.
///
/// Connects to a RACFS server REST API. All methods are async. Paths are
/// absolute (e.g. `/memfs/foo`). Use [`health`](Client::health) to check
/// connectivity and [`login`](Client::login) / [`set_token`](Client::set_token) for JWT auth.
///
/// The client uses reqwest's connection pool (enabled by default). For high
/// concurrency, use [`Client::builder`](Client::builder) to set
/// [`pool_max_idle_per_host`](ClientBuilder::pool_max_idle_per_host).
pub struct Client {
    base_url: String,
    http_client: ReqwestClient,
    auth: Arc<RwLock<AuthState>>,
    circuit: Arc<RwLock<CircuitState>>,
}

/// Builder for [`Client`] with optional connection pool tuning.
#[derive(Default)]
pub struct ClientBuilder {
    pool_max_idle_per_host: Option<usize>,
    pool_idle_timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Create a new builder with default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of idle connections per host to keep in the pool.
    /// Default (reqwest) is 10. Increase for high concurrency (e.g. 32 or 64).
    pub fn pool_max_idle_per_host(mut self, n: usize) -> Self {
        self.pool_max_idle_per_host = Some(n);
        self
    }

    /// Set how long an idle connection is kept in the pool before being closed.
    /// Default (reqwest) is 90 seconds.
    pub fn pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_idle_timeout = Some(timeout);
        self
    }

    /// Build the client for the given server base URL.
    pub fn build(self, base_url: impl Into<String>) -> Result<Client, Error> {
        let mut b = ReqwestClient::builder();
        if let Some(n) = self.pool_max_idle_per_host {
            b = b.pool_max_idle_per_host(n);
        }
        if let Some(t) = self.pool_idle_timeout {
            b = b.pool_idle_timeout(t);
        }
        let http_client = b.build().map_err(|e| Error::Http { source: e })?;
        Ok(Client {
            base_url: base_url.into(),
            http_client,
            auth: Arc::new(RwLock::new(AuthState::default())),
            circuit: Arc::new(RwLock::new(CircuitState::default())),
        })
    }
}

impl Client {
    /// Create a new client with default connection pool settings.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client: ReqwestClient::new(),
            auth: Arc::new(RwLock::new(AuthState::default())),
            circuit: Arc::new(RwLock::new(CircuitState::default())),
        }
    }

    /// Return a builder to configure connection pool options (e.g. for high concurrency).
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Set the authentication token
    pub fn set_token(&self, token: impl Into<String>) {
        let mut auth = self.auth.write();
        auth.token = Some(token.into());
    }

    /// Get the authentication token
    pub fn token(&self) -> Option<String> {
        let auth = self.auth.read();
        auth.token.clone()
    }

    /// Check if authenticated
    pub fn is_authenticated(&self) -> bool {
        let auth = self.auth.read();
        auth.token.is_some()
    }

    /// Clear the authentication token
    pub fn logout(&self) {
        let mut auth = self.auth.write();
        auth.token = None;
    }

    /// Build request with auth header if token is set
    fn build_request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let auth = self.auth.read();
        match &auth.token {
            Some(token) => request.header("Authorization", format!("Bearer {}", token)),
            None => request,
        }
    }

    /// Send a request with retries on connection and timeout errors.
    /// Uses exponential backoff (initial_backoff * 2^attempt).
    /// Fails fast with [`Error::CircuitOpen`] when the circuit breaker is open.
    async fn send_with_retry<F>(&self, build: F) -> Result<reqwest::Response, Error>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let now = Instant::now();
        {
            let mut c = self.circuit.write();
            if let Some(open_until) = c.open_until {
                if now < open_until {
                    return Err(Error::CircuitOpen);
                }
                // Cooldown elapsed -> half-open: allow one request
                c.open_until = None;
                c.failure_count = 0;
            }
        }

        let mut last_err = None;
        for attempt in 0..=DEFAULT_MAX_RETRIES {
            match build().send().await {
                Ok(r) => {
                    self.circuit.write().failure_count = 0;
                    return Ok(r);
                }
                Err(e) => {
                    if (e.is_connect() || e.is_timeout()) && attempt < DEFAULT_MAX_RETRIES {
                        last_err = Some(e);
                        let backoff = Duration::from_millis(
                            DEFAULT_INITIAL_BACKOFF_MS * 2u64.saturating_pow(attempt),
                        );
                        tokio::time::sleep(backoff).await;
                    } else {
                        self.record_circuit_failure();
                        return Err(Error::Http { source: e });
                    }
                }
            }
        }
        if let Some(err) = last_err {
            self.record_circuit_failure();
            Err(Error::Http { source: err })
        } else {
            Err(Error::Internal {
                message: "retry loop exited without capturing error".to_string(),
            })
        }
    }

    fn record_circuit_failure(&self) {
        let mut c = self.circuit.write();
        c.failure_count += 1;
        if c.failure_count >= DEFAULT_CIRCUIT_FAILURE_THRESHOLD {
            c.open_until =
                Some(Instant::now() + Duration::from_secs(DEFAULT_CIRCUIT_OPEN_DURATION_SECS));
        }
    }

    /// Login with username and password
    pub async fn login(&self, username: &str, password: &str) -> Result<LoginResponse, Error> {
        let url = format!("{}/api/v1/auth/login", self.base_url);
        let body = LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        };

        let response = self
            .send_with_retry(|| self.http_client.post(&url).json(&body))
            .await?;

        let login_response: LoginResponse = Self::handle_response(response).await?;

        // Store the token
        self.set_token(&login_response.access_token);

        Ok(login_response)
    }

    /// Get current user info
    pub async fn me(&self) -> Result<UserResponse, Error> {
        let url = format!("{}/api/v1/auth/me", self.base_url);
        let response = self
            .send_with_retry(|| self.build_request(self.http_client.get(&url)))
            .await?;

        Self::handle_response(response).await
    }

    /// Refresh the authentication token
    pub async fn refresh(&self) -> Result<LoginResponse, Error> {
        let url = format!("{}/api/v1/auth/refresh", self.base_url);
        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url)))
            .await?;

        let login_response: LoginResponse = Self::handle_response(response).await?;

        // Store the new token
        self.set_token(&login_response.access_token);

        Ok(login_response)
    }

    /// Check response status and handle errors
    async fn handle_response<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, Error> {
        let status = response.status();

        if status.is_success() {
            response
                .json::<T>()
                .await
                .map_err(|source| Error::Http { source })
        } else {
            // Try to parse as HttpErrorResponse
            let error_response = response
                .json::<racfs_http_error::HttpErrorResponse>()
                .await
                .unwrap_or_else(|_| {
                    racfs_http_error::HttpErrorResponse::new(
                        racfs_http_error::ErrorCode::InternalServerError,
                        format!("HTTP {} error", status),
                    )
                });

            Err(Error::Api {
                code: error_response.code.to_string(),
                message: error_response.message,
                detail: error_response.detail.clone(),
            })
        }
    }

    /// Handle response that returns plain text
    async fn handle_text_response(response: reqwest::Response) -> Result<String, Error> {
        let status = response.status();

        if status.is_success() {
            response
                .text()
                .await
                .map_err(|source| Error::Http { source })
        } else {
            let error_response = response
                .json::<racfs_http_error::HttpErrorResponse>()
                .await
                .unwrap_or_else(|_| {
                    racfs_http_error::HttpErrorResponse::new(
                        racfs_http_error::ErrorCode::InternalServerError,
                        format!("HTTP {} error", status),
                    )
                });

            Err(Error::Api {
                code: error_response.code.to_string(),
                message: error_response.message,
                detail: error_response.detail.clone(),
            })
        }
    }

    /// Check health
    pub async fn health(&self) -> Result<HealthResponse, Error> {
        let url = format!("{}/api/v1/health", self.base_url);
        let response = self
            .send_with_retry(|| self.build_request(self.http_client.get(&url)))
            .await?;

        Self::handle_response(response).await
    }

    /// Get capabilities
    pub async fn capabilities(&self) -> Result<CapabilitiesResponse, Error> {
        let url = format!("{}/api/v1/capabilities", self.base_url);
        let response = self
            .send_with_retry(|| self.build_request(self.http_client.get(&url)))
            .await?;

        Self::handle_response(response).await
    }

    /// Read a file
    pub async fn read_file(&self, path: &str) -> Result<String, Error> {
        let url = format!("{}/api/v1/files", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(self.http_client.get(&url).query(&[("path", path)]))
            })
            .await?;

        Self::handle_text_response(response).await
    }

    /// Write to a file
    pub async fn write_file(
        &self,
        path: &str,
        data: &str,
        offset: Option<i64>,
    ) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/files", self.base_url);
        let body = WriteRequest {
            path: path.to_string(),
            data: data.to_string(),
            offset,
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.put(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// Create a file
    pub async fn create_file(&self, path: &str) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/files", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(self.http_client.post(&url).query(&[("path", path)]))
            })
            .await?;

        Self::handle_response(response).await
    }

    /// Delete a file
    pub async fn remove(&self, path: &str) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/files", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(self.http_client.delete(&url).query(&[("path", path)]))
            })
            .await?;

        Self::handle_response(response).await
    }

    /// Create a directory
    pub async fn mkdir(&self, path: &str, perm: Option<u32>) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/directories", self.base_url);
        let body = CreateDirectoryRequest {
            path: path.to_string(),
            perm,
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// List directory contents
    pub async fn read_dir(&self, path: &str) -> Result<DirectoryListResponse, Error> {
        let url = format!("{}/api/v1/directories", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(self.http_client.get(&url).query(&[("path", path)]))
            })
            .await?;

        Self::handle_response(response).await
    }

    /// Get file metadata
    pub async fn stat(&self, path: &str) -> Result<FileMetadataResponse, Error> {
        let url = format!("{}/api/v1/stat", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(self.http_client.get(&url).query(&[("path", path)]))
            })
            .await?;

        Self::handle_response(response).await
    }

    /// Rename/move a file or directory
    pub async fn rename(&self, old_path: &str, new_path: &str) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/rename", self.base_url);
        let body = RenameRequest {
            old_path: old_path.to_string(),
            new_path: new_path.to_string(),
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// Change file permissions
    pub async fn chmod(&self, path: &str, mode: u32) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/chmod", self.base_url);
        let body = ChmodRequest {
            path: path.to_string(),
            mode,
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// Truncate file to size bytes
    pub async fn truncate(&self, path: &str, size: u64) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/truncate", self.base_url);
        let body = TruncateRequest {
            path: path.to_string(),
            size,
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// Create a symbolic link at link_path pointing to target.
    /// Fails if the backend does not support symlinks.
    pub async fn symlink(&self, target: &str, link_path: &str) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/symlink", self.base_url);
        let body = SymlinkRequest {
            target: target.to_string(),
            link_path: link_path.to_string(),
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// Get an extended attribute value (base64-encoded).
    pub async fn get_xattr(&self, path: &str, name: &str) -> Result<XattrValueResponse, Error> {
        let url = format!("{}/api/v1/xattr", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(
                    self.http_client
                        .get(&url)
                        .query(&[("path", path), ("name", name)]),
                )
            })
            .await?;

        Self::handle_response(response).await
    }

    /// Set an extended attribute. Value must be base64-encoded.
    pub async fn set_xattr(
        &self,
        path: &str,
        name: &str,
        value_base64: &str,
    ) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/xattr", self.base_url);
        let body = SetXattrRequest {
            path: path.to_string(),
            name: name.to_string(),
            value: value_base64.to_string(),
        };

        let response = self
            .send_with_retry(|| self.build_request(self.http_client.post(&url).json(&body)))
            .await?;

        Self::handle_response(response).await
    }

    /// List extended attribute names for a path.
    pub async fn list_xattr(&self, path: &str) -> Result<XattrListResponse, Error> {
        let url = format!("{}/api/v1/xattr/list", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(self.http_client.get(&url).query(&[("path", path)]))
            })
            .await?;

        Self::handle_response(response).await
    }

    /// Remove an extended attribute.
    pub async fn remove_xattr(&self, path: &str, name: &str) -> Result<MessageResponse, Error> {
        let url = format!("{}/api/v1/xattr", self.base_url);
        let response = self
            .send_with_retry(|| {
                self.build_request(
                    self.http_client
                        .delete(&url)
                        .query(&[("path", path), ("name", name)]),
                )
            })
            .await?;

        Self::handle_response(response).await
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new_with_trailing_slash() {
        let client = Client::new("http://localhost:8080/");
        assert_eq!(client.base_url(), "http://localhost:8080/");
    }

    #[test]
    fn test_client_new_without_trailing_slash() {
        let client = Client::new("http://localhost:8080");
        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_error_api_display() {
        let error = Error::Api {
            code: "NotFound".to_string(),
            message: "File not found".to_string(),
            detail: None,
        };
        assert!(error.to_string().contains("API error"));
        assert!(error.to_string().contains("NotFound"));
        assert!(error.to_string().contains("File not found"));
    }

    #[test]
    fn test_client_builder_pool_options() {
        let client = Client::builder()
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(60))
            .build("http://localhost:8080")
            .expect("test: reqwest client build");
        assert_eq!(client.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_error_circuit_open_display() {
        let e = Error::CircuitOpen;
        let s = e.to_string();
        assert!(s.contains("Circuit breaker"));
        assert!(s.contains("cooldown"));
    }

    #[test]
    fn test_error_not_authenticated_display() {
        let e = Error::NotAuthenticated;
        assert!(e.to_string().contains("Not authenticated"));
    }

    #[test]
    fn test_error_internal_display() {
        let e = Error::Internal {
            message: "test message".to_string(),
        };
        let s = e.to_string();
        assert!(s.contains("Internal"));
        assert!(s.contains("test message"));
    }

    #[test]
    fn test_auth_set_token_and_get() {
        let client = Client::new("http://localhost:8080");
        assert!(!client.is_authenticated());
        assert!(client.token().is_none());
        client.set_token("my-jwt-token");
        assert!(client.is_authenticated());
        assert_eq!(client.token().as_deref(), Some("my-jwt-token"));
    }

    #[test]
    fn test_auth_logout_clears_token() {
        let client = Client::new("http://localhost:8080");
        client.set_token("token");
        assert!(client.is_authenticated());
        client.logout();
        assert!(!client.is_authenticated());
        assert!(client.token().is_none());
    }
}
