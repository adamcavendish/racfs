//! Rate limiting middleware for RACFS
//!
//! Limits requests per client identifier: authenticated user (Bearer token),
//! API key (X-API-Key header), or IP when unauthenticated.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
    time::{Duration, Instant},
};

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
    /// Headers to include
    pub headers: RateLimitHeaders,
}

/// Which headers to include in responses
#[derive(Debug, Clone)]
pub struct RateLimitHeaders {
    /// Include X-RateLimit-Limit header
    pub limit: bool,
    /// Include X-RateLimit-Remaining header
    pub remaining: bool,
    /// Include X-RateLimit-Reset header
    pub reset: bool,
}

impl Default for RateLimitHeaders {
    fn default() -> Self {
        Self {
            limit: true,
            remaining: true,
            reset: true,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            headers: RateLimitHeaders::default(),
        }
    }
}

/// Per-user rate limit state
#[derive(Debug)]
struct UserRateLimit {
    /// Request count in current window
    count: u32,
    /// Window start time
    window_start: Instant,
}

/// Rate limiter state
#[derive(Debug)]
pub struct RateLimiter {
    /// Per-user rate limit state
    users: Arc<RwLock<HashMap<String, UserRateLimit>>>,
    /// Configuration (reloadable via hot reload)
    config: Arc<RwLock<RateLimitConfig>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Update configuration (e.g. on SIGHUP reload). New limits apply to subsequent requests.
    pub fn update_config(&self, config: RateLimitConfig) {
        *self.config.write() = config;
    }

    /// Check rate limit for a client identifier
    /// Returns (allowed, remaining, reset_time)
    pub fn check(&self, client_id: &str) -> (bool, u32, u64) {
        let mut users = self.users.write();
        let config = self.config.read();
        let now = Instant::now();

        // Get or create user state
        let user_state = users
            .entry(client_id.to_string())
            .or_insert_with(|| UserRateLimit {
                count: 0,
                window_start: now,
            });

        // Check if window has expired
        if now.duration_since(user_state.window_start) >= config.window {
            // Reset the window
            user_state.count = 0;
            user_state.window_start = now;
        }

        // Check if limit exceeded
        let remaining = config.max_requests.saturating_sub(user_state.count);
        let allowed = user_state.count < config.max_requests;

        if allowed {
            user_state.count += 1;
        }

        // Calculate reset time (seconds until window end)
        let elapsed = now.duration_since(user_state.window_start);
        let reset_time = config.window.saturating_sub(elapsed).as_secs();

        (allowed, remaining, reset_time)
    }

    /// Add rate limit headers to response
    pub fn add_headers(&self, response: &mut Response, remaining: u32, reset: u64) {
        let config = self.config.read();
        let headers = response.headers_mut();

        if config.headers.limit
            && let Ok(value) = HeaderValue::from_str(&config.max_requests.to_string())
        {
            headers.insert("x-rate-limit-limit", value);
        }

        if config.headers.remaining
            && let Ok(value) = HeaderValue::from_str(&remaining.to_string())
        {
            headers.insert("x-rate-limit-remaining", value);
        }

        if config.headers.reset
            && let Ok(value) = HeaderValue::from_str(&reset.to_string())
        {
            headers.insert("x-rate-limit-reset", value);
        }
    }

    /// Get client identifier from request for rate limiting.
    /// Uses, in order: X-API-Key header (per API key), Authorization header (per user/token), then IP.
    pub fn get_client_id(request: &Request<Body>) -> String {
        if let Some(api_key) = request.headers().get("x-api-key")
            && let Ok(key_str) = api_key.to_str()
        {
            return Self::client_id_apikey(key_str);
        }

        if let Some(auth) = request.headers().get("authorization")
            && let Ok(auth_str) = auth.to_str()
        {
            return Self::client_id_user(auth_str);
        }

        if let Some(remote_addr) = request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        {
            return format!("ip:{}", remote_addr.0.ip());
        }

        "default".to_string()
    }

    fn hash_str(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Client id for API key (hashed so we don't store raw keys in state).
    fn client_id_apikey(key: &str) -> String {
        format!("apikey:{}", Self::hash_str(key))
    }

    /// Client id for Bearer/user auth (hashed so we don't store tokens in state).
    fn client_id_user(auth: &str) -> String {
        format!("user:{}", Self::hash_str(auth))
    }
}

/// Rate limit error response
#[derive(serde::Serialize)]
struct RateLimitErrorResponse {
    code: String,
    message: String,
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(request: Request<Body>, next: Next) -> Response {
    // Get rate limiter from request extensions
    let limiter = match request.extensions().get::<Arc<RateLimiter>>() {
        Some(l) => l.clone(),
        None => {
            // No rate limiter configured, skip
            return next.run(request).await;
        }
    };

    let client_id = RateLimiter::get_client_id(&request);
    let (allowed, remaining, reset) = limiter.check(&client_id);

    let mut response = next.run(request).await;

    if allowed {
        // Add headers even for successful requests
        limiter.add_headers(&mut response, remaining, reset);
    } else {
        // Rate limited - return 429
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        limiter.add_headers(&mut response, 0, reset);

        // Add error body
        let body = serde_json::to_string(&RateLimitErrorResponse {
            code: "RateLimitExceeded".to_string(),
            message: format!("Rate limit exceeded. Try again in {} seconds.", reset),
        })
        .unwrap_or_default();

        *response.body_mut() = Body::from(body);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ConnectInfo;
    use std::net::SocketAddr;

    #[test]
    fn test_client_id_apikey_prefix() {
        let id = RateLimiter::client_id_apikey("secret-key");
        assert!(id.starts_with("apikey:"));
        assert_eq!(id, RateLimiter::client_id_apikey("secret-key"));
    }

    #[test]
    fn test_client_id_user_prefix() {
        let id = RateLimiter::client_id_user("Bearer eyJhbGc...");
        assert!(id.starts_with("user:"));
        assert_eq!(id, RateLimiter::client_id_user("Bearer eyJhbGc..."));
    }

    #[test]
    fn rate_limit_headers_default() {
        let h = RateLimitHeaders::default();
        assert!(h.limit);
        assert!(h.remaining);
        assert!(h.reset);
    }

    #[test]
    fn rate_limit_config_default() {
        let c = RateLimitConfig::default();
        assert_eq!(c.max_requests, 100);
        assert_eq!(c.window, Duration::from_secs(60));
    }

    #[test]
    fn rate_limiter_new_and_check_allowed_until_limit() {
        let config = RateLimitConfig {
            max_requests: 2,
            window: Duration::from_secs(60),
            headers: RateLimitHeaders::default(),
        };
        let limiter = RateLimiter::new(config);

        let (allowed1, remaining1, _) = limiter.check("user1");
        assert!(allowed1);
        assert_eq!(remaining1, 2); // remaining before this request (2-0)

        let (allowed2, remaining2, _) = limiter.check("user1");
        assert!(allowed2);
        assert_eq!(remaining2, 1); // remaining before this request (2-1)

        let (allowed3, _, _) = limiter.check("user1");
        assert!(!allowed3);
    }

    #[test]
    fn rate_limiter_per_client_id() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
            headers: RateLimitHeaders::default(),
        };
        let limiter = RateLimiter::new(config);

        let (a1, _, _) = limiter.check("alice");
        let (a2, _, _) = limiter.check("bob");
        assert!(a1);
        assert!(a2);

        let (a3, _, _) = limiter.check("alice");
        assert!(!a3);
    }

    #[test]
    fn rate_limiter_update_config() {
        let config = RateLimitConfig {
            max_requests: 1,
            window: Duration::from_secs(60),
            headers: RateLimitHeaders::default(),
        };
        let limiter = RateLimiter::new(config);

        let (_, _, _) = limiter.check("u");
        let (allowed, _, _) = limiter.check("u");
        assert!(!allowed);

        limiter.update_config(RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
            headers: RateLimitHeaders::default(),
        });

        let (allowed2, remaining, _) = limiter.check("u");
        assert!(allowed2);
        assert!(remaining <= 2, "remaining should reflect new limit");
    }

    #[test]
    fn rate_limiter_add_headers() {
        let config = RateLimitConfig {
            max_requests: 10,
            window: Duration::from_secs(60),
            headers: RateLimitHeaders::default(),
        };
        let limiter = RateLimiter::new(config);
        let mut response = Response::new(Body::empty());

        limiter.add_headers(&mut response, 5, 42);

        let headers = response.headers();
        assert_eq!(
            headers
                .get("x-rate-limit-limit")
                .and_then(|v| v.to_str().ok()),
            Some("10")
        );
        assert_eq!(
            headers
                .get("x-rate-limit-remaining")
                .and_then(|v| v.to_str().ok()),
            Some("5")
        );
        assert_eq!(
            headers
                .get("x-rate-limit-reset")
                .and_then(|v| v.to_str().ok()),
            Some("42")
        );
    }

    #[test]
    fn rate_limiter_add_headers_respects_config() {
        let config = RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(30),
            headers: RateLimitHeaders {
                limit: true,
                remaining: false,
                reset: false,
            },
        };
        let limiter = RateLimiter::new(config);
        let mut response = Response::new(Body::empty());

        limiter.add_headers(&mut response, 2, 10);

        let headers = response.headers();
        assert!(headers.get("x-rate-limit-limit").is_some());
        assert!(headers.get("x-rate-limit-remaining").is_none());
        assert!(headers.get("x-rate-limit-reset").is_none());
    }

    #[test]
    fn get_client_id_from_x_api_key() {
        let req = Request::builder()
            .header("x-api-key", "my-key")
            .body(Body::empty())
            .unwrap();
        let id = RateLimiter::get_client_id(&req);
        assert!(id.starts_with("apikey:"));
    }

    #[test]
    fn get_client_id_from_authorization() {
        let req = Request::builder()
            .header("authorization", "Bearer token123")
            .body(Body::empty())
            .unwrap();
        let id = RateLimiter::get_client_id(&req);
        assert!(id.starts_with("user:"));
    }

    #[test]
    fn get_client_id_prefers_api_key_over_authorization() {
        let req = Request::builder()
            .header("x-api-key", "k")
            .header("authorization", "Bearer t")
            .body(Body::empty())
            .unwrap();
        let id = RateLimiter::get_client_id(&req);
        assert!(id.starts_with("apikey:"));
    }

    #[test]
    fn get_client_id_from_connect_info() {
        let mut req = Request::builder().body(Body::empty()).unwrap();
        let addr = SocketAddr::from(([192, 168, 1, 1], 12345));
        req.extensions_mut().insert(ConnectInfo(addr));
        let id = RateLimiter::get_client_id(&req);
        assert_eq!(id, "ip:192.168.1.1");
    }

    #[test]
    fn get_client_id_default_when_no_headers_or_connect_info() {
        let req = Request::builder().body(Body::empty()).unwrap();
        let id = RateLimiter::get_client_id(&req);
        assert_eq!(id, "default");
    }
}
