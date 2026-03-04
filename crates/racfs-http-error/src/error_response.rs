//! HTTP Error Response type

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ErrorCode;

/// Standardized HTTP error response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HttpErrorResponse {
    /// Error code
    pub code: ErrorCode,
    /// Human-readable error message
    pub message: String,
    /// Optional context (e.g. operation and path) for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl HttpErrorResponse {
    /// Create a new error response
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// Create an error response with optional context detail
    pub fn new_with_detail(
        code: ErrorCode,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            detail: Some(detail.into()),
        }
    }

    /// Create a bad request error
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    /// Create a not found error
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Create an internal server error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalServerError, message)
    }
}

impl IntoResponse for HttpErrorResponse {
    fn into_response(self) -> Response {
        let status = StatusCode::from(self.code.clone());
        (status, Json(self)).into_response()
    }
}

impl std::fmt::Display for HttpErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)?;
        if let Some(ref d) = self.detail {
            write!(f, " ({})", d)?;
        }
        Ok(())
    }
}

impl std::error::Error for HttpErrorResponse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_code_and_message_no_detail() {
        let r = HttpErrorResponse::new(ErrorCode::NotFound, "file missing");
        assert_eq!(r.code, ErrorCode::NotFound);
        assert_eq!(r.message, "file missing");
        assert!(r.detail.is_none());
    }

    #[test]
    fn new_with_detail_sets_detail() {
        let r = HttpErrorResponse::new_with_detail(
            ErrorCode::BadRequest,
            "invalid path",
            "path: /foo/../bar",
        );
        assert_eq!(r.code, ErrorCode::BadRequest);
        assert_eq!(r.message, "invalid path");
        assert_eq!(r.detail.as_deref(), Some("path: /foo/../bar"));
    }

    #[test]
    fn bad_request_helper() {
        let r = HttpErrorResponse::bad_request("bad input");
        assert_eq!(r.code, ErrorCode::BadRequest);
        assert_eq!(r.message, "bad input");
    }

    #[test]
    fn not_found_helper() {
        let r = HttpErrorResponse::not_found("resource");
        assert_eq!(r.code, ErrorCode::NotFound);
        assert_eq!(r.message, "resource");
    }

    #[test]
    fn internal_helper() {
        let r = HttpErrorResponse::internal("server error");
        assert_eq!(r.code, ErrorCode::InternalServerError);
        assert_eq!(r.message, "server error");
    }

    #[test]
    fn display_without_detail() {
        let r = HttpErrorResponse::new(ErrorCode::NotFound, "x");
        let s = format!("{}", r);
        assert!(s.contains("NotFound"));
        assert!(s.contains("x"));
        assert!(!s.contains('('));
    }

    #[test]
    fn display_with_detail() {
        let r = HttpErrorResponse::new_with_detail(ErrorCode::Forbidden, "denied", "op: read");
        let s = format!("{}", r);
        assert!(s.contains("Forbidden"));
        assert!(s.contains("denied"));
        assert!(s.contains("op: read"));
    }

    #[test]
    fn is_std_error() {
        let r = HttpErrorResponse::not_found("x");
        let _: &dyn std::error::Error = &r;
    }

    #[test]
    fn into_response_uses_correct_status() {
        let r = HttpErrorResponse::new(ErrorCode::NotFound, "msg");
        let res = r.into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
