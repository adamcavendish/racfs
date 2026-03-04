//! Authentication errors.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Authentication errors.
#[derive(Debug, snafu::Snafu)]
#[snafu(visibility(pub))]
pub enum AuthError {
    /// Invalid credentials
    #[snafu(display("Invalid username or password"))]
    InvalidCredentials,

    /// Invalid or expired token
    #[snafu(display("Invalid or expired token"))]
    InvalidToken,

    /// Token validation failed
    #[snafu(display("Token validation failed: {}", source))]
    TokenValidation { source: jsonwebtoken::errors::Error },

    /// Password verification failed
    #[snafu(display("Password verification failed"))]
    PasswordMismatch,

    /// User not found
    #[snafu(display("User not found"))]
    UserNotFound,

    /// User already exists
    #[snafu(display("User already exists"))]
    UserAlreadyExists,

    /// Invalid API key
    #[snafu(display("Invalid API key"))]
    InvalidApiKey,

    /// Unauthorized - missing authentication
    #[snafu(display("Unauthorized"))]
    Unauthorized,

    /// Forbidden - insufficient permissions
    #[snafu(display("Forbidden: insufficient permissions"))]
    Forbidden,

    /// Internal server error
    #[snafu(display("Internal server error: {}", source))]
    Internal { source: Box<dyn std::error::Error> },
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::TokenValidation { .. } => {
                (StatusCode::UNAUTHORIZED, "Token validation failed")
            }
            AuthError::PasswordMismatch => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            AuthError::UserNotFound => (StatusCode::NOT_FOUND, "User not found"),
            AuthError::UserAlreadyExists => (StatusCode::CONFLICT, "User already exists"),
            AuthError::InvalidApiKey => (StatusCode::UNAUTHORIZED, "Invalid API key"),
            AuthError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AuthError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden: insufficient permissions"),
            AuthError::Internal { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn status_and_body(err: AuthError) -> (StatusCode, String) {
        let resp = err.into_response();
        let status = resp.status();
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let s = String::from_utf8(body.to_vec()).unwrap();
        (status, s)
    }

    #[tokio::test]
    async fn invalid_credentials_returns_401() {
        let (status, body) = status_and_body(AuthError::InvalidCredentials).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.contains("Invalid credentials"));
    }

    #[tokio::test]
    async fn invalid_token_returns_401() {
        let (status, _) = status_and_body(AuthError::InvalidToken).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn unauthorized_returns_401() {
        let (status, _) = status_and_body(AuthError::Unauthorized).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn forbidden_returns_403() {
        let (status, body) = status_and_body(AuthError::Forbidden).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(body.contains("Forbidden"));
    }

    #[tokio::test]
    async fn user_not_found_returns_404() {
        let (status, _) = status_and_body(AuthError::UserNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn user_already_exists_returns_409() {
        let (status, _) = status_and_body(AuthError::UserAlreadyExists).await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn invalid_api_key_returns_401() {
        let (status, _) = status_and_body(AuthError::InvalidApiKey).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_error_display() {
        assert!(
            AuthError::InvalidCredentials
                .to_string()
                .contains("Invalid")
        );
        assert!(AuthError::Unauthorized.to_string().contains("Unauthorized"));
    }
}
