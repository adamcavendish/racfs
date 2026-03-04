//! HTTP middleware for RACFS: latency recording, etc.

use axum::{
    body::Body,
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Map (method, path) to a stable operation name for metrics labels.
pub(crate) fn operation_from_request(method: &Method, path: &str) -> &'static str {
    match (method.as_str(), path) {
        ("GET", "/api/v1/files") => "read_file",
        ("POST", "/api/v1/files") => "create_file",
        ("PUT", "/api/v1/files") => "write_file",
        ("DELETE", "/api/v1/files") => "delete_file",
        ("GET", "/api/v1/directories") => "list_directory",
        ("POST", "/api/v1/directories") => "create_directory",
        ("GET", "/api/v1/stat") => "stat",
        ("POST", "/api/v1/rename") => "rename",
        ("POST", "/api/v1/chmod") => "chmod",
        ("POST", "/api/v1/truncate") => "truncate",
        ("GET", "/api/v1/health") => "health",
        ("GET", "/metrics") => "metrics",
        _ => "other",
    }
}

/// Records request duration in `racfs_request_duration_seconds` histogram by operation.
/// Use with `axum::middleware::from_fn_with_state(state.clone(), latency_middleware)`.
pub async fn latency_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = std::time::Instant::now();
    let path = request.uri().path().to_string();
    let method = request.method().clone();
    let response = next.run(request).await;
    let elapsed_secs = start.elapsed().as_secs_f64();
    let operation = operation_from_request(&method, &path);
    state
        .vfs_metrics
        .request_duration_seconds
        .with_label_values(&[operation])
        .observe(elapsed_secs);
    response
}

#[cfg(test)]
mod tests {
    use super::operation_from_request;
    use axum::http::Method;

    #[test]
    fn operation_read_file() {
        assert_eq!(
            operation_from_request(&Method::GET, "/api/v1/files"),
            "read_file"
        );
    }

    #[test]
    fn operation_create_file() {
        assert_eq!(
            operation_from_request(&Method::POST, "/api/v1/files"),
            "create_file"
        );
    }

    #[test]
    fn operation_write_file() {
        assert_eq!(
            operation_from_request(&Method::PUT, "/api/v1/files"),
            "write_file"
        );
    }

    #[test]
    fn operation_delete_file() {
        assert_eq!(
            operation_from_request(&Method::DELETE, "/api/v1/files"),
            "delete_file"
        );
    }

    #[test]
    fn operation_list_directory() {
        assert_eq!(
            operation_from_request(&Method::GET, "/api/v1/directories"),
            "list_directory"
        );
    }

    #[test]
    fn operation_create_directory() {
        assert_eq!(
            operation_from_request(&Method::POST, "/api/v1/directories"),
            "create_directory"
        );
    }

    #[test]
    fn operation_stat() {
        assert_eq!(operation_from_request(&Method::GET, "/api/v1/stat"), "stat");
    }

    #[test]
    fn operation_rename() {
        assert_eq!(
            operation_from_request(&Method::POST, "/api/v1/rename"),
            "rename"
        );
    }

    #[test]
    fn operation_chmod() {
        assert_eq!(
            operation_from_request(&Method::POST, "/api/v1/chmod"),
            "chmod"
        );
    }

    #[test]
    fn operation_truncate() {
        assert_eq!(
            operation_from_request(&Method::POST, "/api/v1/truncate"),
            "truncate"
        );
    }

    #[test]
    fn operation_health() {
        assert_eq!(
            operation_from_request(&Method::GET, "/api/v1/health"),
            "health"
        );
    }

    #[test]
    fn operation_metrics() {
        assert_eq!(operation_from_request(&Method::GET, "/metrics"), "metrics");
    }

    #[test]
    fn operation_other_unknown_path() {
        assert_eq!(
            operation_from_request(&Method::GET, "/api/v1/other"),
            "other"
        );
    }

    #[test]
    fn operation_other_unknown_method() {
        assert_eq!(
            operation_from_request(&Method::PATCH, "/api/v1/files"),
            "other"
        );
    }
}
