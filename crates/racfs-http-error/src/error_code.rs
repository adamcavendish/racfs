//! Error codes for HTTP responses

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Error codes for HTTP responses
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum ErrorCode {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    InternalServerError,
    NotImplemented,
    ServiceUnavailable,
    GatewayTimeout,
    InsufficientStorage,
}

impl From<ErrorCode> for StatusCode {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::NotImplemented => StatusCode::NOT_IMPLEMENTED,
            ErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::GatewayTimeout => StatusCode::GATEWAY_TIMEOUT,
            ErrorCode::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ErrorCode::BadRequest => "BadRequest",
            ErrorCode::Unauthorized => "Unauthorized",
            ErrorCode::Forbidden => "Forbidden",
            ErrorCode::NotFound => "NotFound",
            ErrorCode::Conflict => "Conflict",
            ErrorCode::InternalServerError => "InternalServerError",
            ErrorCode::NotImplemented => "NotImplemented",
            ErrorCode::ServiceUnavailable => "ServiceUnavailable",
            ErrorCode::GatewayTimeout => "GatewayTimeout",
            ErrorCode::InsufficientStorage => "InsufficientStorage",
        };
        f.write_str(s)
    }
}

impl From<&str> for ErrorCode {
    fn from(s: &str) -> Self {
        match s {
            "BadRequest" => ErrorCode::BadRequest,
            "Unauthorized" => ErrorCode::Unauthorized,
            "Forbidden" => ErrorCode::Forbidden,
            "NotFound" => ErrorCode::NotFound,
            "Conflict" => ErrorCode::Conflict,
            "InternalServerError" => ErrorCode::InternalServerError,
            "NotImplemented" => ErrorCode::NotImplemented,
            "ServiceUnavailable" => ErrorCode::ServiceUnavailable,
            "GatewayTimeout" => ErrorCode::GatewayTimeout,
            "InsufficientStorage" => ErrorCode::InsufficientStorage,
            _ => ErrorCode::InternalServerError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_to_status_bad_request() {
        assert_eq!(
            StatusCode::from(ErrorCode::BadRequest),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn error_code_to_status_unauthorized() {
        assert_eq!(
            StatusCode::from(ErrorCode::Unauthorized),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn error_code_to_status_forbidden() {
        assert_eq!(
            StatusCode::from(ErrorCode::Forbidden),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn error_code_to_status_not_found() {
        assert_eq!(StatusCode::from(ErrorCode::NotFound), StatusCode::NOT_FOUND);
    }

    #[test]
    fn error_code_to_status_conflict() {
        assert_eq!(StatusCode::from(ErrorCode::Conflict), StatusCode::CONFLICT);
    }

    #[test]
    fn error_code_to_status_internal_server_error() {
        assert_eq!(
            StatusCode::from(ErrorCode::InternalServerError),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn error_code_to_status_not_implemented() {
        assert_eq!(
            StatusCode::from(ErrorCode::NotImplemented),
            StatusCode::NOT_IMPLEMENTED
        );
    }

    #[test]
    fn error_code_to_status_service_unavailable() {
        assert_eq!(
            StatusCode::from(ErrorCode::ServiceUnavailable),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn error_code_to_status_gateway_timeout() {
        assert_eq!(
            StatusCode::from(ErrorCode::GatewayTimeout),
            StatusCode::GATEWAY_TIMEOUT
        );
    }

    #[test]
    fn error_code_to_status_insufficient_storage() {
        assert_eq!(
            StatusCode::from(ErrorCode::InsufficientStorage),
            StatusCode::INSUFFICIENT_STORAGE
        );
    }

    #[test]
    fn error_code_display() {
        assert_eq!(ErrorCode::BadRequest.to_string(), "BadRequest");
        assert_eq!(ErrorCode::NotFound.to_string(), "NotFound");
        assert_eq!(ErrorCode::GatewayTimeout.to_string(), "GatewayTimeout");
    }

    #[test]
    fn error_code_display_all_variants() {
        assert_eq!(ErrorCode::Unauthorized.to_string(), "Unauthorized");
        assert_eq!(ErrorCode::Forbidden.to_string(), "Forbidden");
        assert_eq!(ErrorCode::Conflict.to_string(), "Conflict");
        assert_eq!(
            ErrorCode::InternalServerError.to_string(),
            "InternalServerError"
        );
        assert_eq!(ErrorCode::NotImplemented.to_string(), "NotImplemented");
        assert_eq!(
            ErrorCode::ServiceUnavailable.to_string(),
            "ServiceUnavailable"
        );
        assert_eq!(
            ErrorCode::InsufficientStorage.to_string(),
            "InsufficientStorage"
        );
    }

    #[test]
    fn error_code_from_str_known() {
        assert_eq!(ErrorCode::from("BadRequest"), ErrorCode::BadRequest);
        assert_eq!(ErrorCode::from("NotFound"), ErrorCode::NotFound);
        assert_eq!(ErrorCode::from("Conflict"), ErrorCode::Conflict);
        assert_eq!(
            ErrorCode::from("InsufficientStorage"),
            ErrorCode::InsufficientStorage
        );
        assert_eq!(ErrorCode::from("Unauthorized"), ErrorCode::Unauthorized);
        assert_eq!(ErrorCode::from("Forbidden"), ErrorCode::Forbidden);
        assert_eq!(
            ErrorCode::from("InternalServerError"),
            ErrorCode::InternalServerError
        );
        assert_eq!(ErrorCode::from("NotImplemented"), ErrorCode::NotImplemented);
        assert_eq!(
            ErrorCode::from("ServiceUnavailable"),
            ErrorCode::ServiceUnavailable
        );
        assert_eq!(ErrorCode::from("GatewayTimeout"), ErrorCode::GatewayTimeout);
    }

    #[test]
    fn error_code_from_str_unknown_defaults_to_internal() {
        assert_eq!(ErrorCode::from("Unknown"), ErrorCode::InternalServerError);
        assert_eq!(ErrorCode::from(""), ErrorCode::InternalServerError);
    }
}
