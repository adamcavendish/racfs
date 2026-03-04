//! Authentication middleware for Axum.

use axum::{extract::FromRequestParts, http::request::Parts};
use serde::Serialize;

use super::error::AuthError;
use super::models::Role;

/// Authenticated user information extracted from request.
#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    /// User ID
    pub user_id: String,
    /// Username
    pub username: String,
    /// Role
    pub role: Role,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try to get from extensions first (set by middleware)
        if let Some(auth_user) = parts.extensions.get::<AuthUser>().cloned() {
            return Ok(auth_user);
        }

        Err(AuthError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;

    #[tokio::test]
    async fn auth_user_extracted_when_in_extensions() {
        let auth_user = AuthUser {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            role: Role::User,
        };
        let request = Request::builder().body(Body::empty()).unwrap();
        let (mut parts, _) = request.into_parts();
        parts.extensions.insert(auth_user.clone());

        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        let extracted = result.unwrap();
        assert_eq!(extracted.user_id, "u1");
        assert_eq!(extracted.username, "alice");
    }

    #[tokio::test]
    async fn auth_user_rejects_unauthorized_when_not_in_extensions() {
        let request = Request::builder().body(Body::empty()).unwrap();
        let (mut parts, _) = request.into_parts();

        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::Unauthorized));
    }
}
