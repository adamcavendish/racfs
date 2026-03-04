//! Authentication API endpoints.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

// Access root auth module
use crate::auth::JwtConfig;
use crate::auth::models::{LoginRequest, LoginResponse};
use crate::auth::{ApiKeyAuth, AuthError, AuthUser, JwtAuth};

/// Auth router state.
#[derive(Clone)]
pub struct AuthApiState {
    jwt: JwtAuth,
    api_key: ApiKeyAuth,
}

impl AuthApiState {
    pub fn new(config: JwtConfig) -> Self {
        Self {
            jwt: JwtAuth::new(config),
            api_key: ApiKeyAuth::new(),
        }
    }

    pub fn jwt(&self) -> &JwtAuth {
        &self.jwt
    }

    pub fn api_key(&self) -> &ApiKeyAuth {
        &self.api_key
    }
}

/// Login request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginQuery {
    /// Use API key instead of password
    #[serde(default)]
    pub use_api_key: bool,
}

/// Auth API routes.
pub fn routes() -> Router<AuthApiState> {
    Router::new()
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/refresh", post(refresh))
}

/// Login endpoint.
///
/// Authenticates user and returns JWT token.
pub async fn login(
    State(state): State<AuthApiState>,
    Query(query): Query<LoginQuery>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    // If API key flag is set, use API key authentication
    if query.use_api_key {
        let user = state.api_key().validate_key(&payload.password)?;
        let response = state.jwt().generate_token(&user)?;
        return Ok(Json(response));
    }

    // Otherwise use password authentication
    let response = state.jwt().authenticate(&payload)?;
    Ok(Json(response))
}

/// Logout endpoint.
///
/// Currently a no-op as JWT tokens are stateless.
pub async fn logout(_user: AuthUser) -> impl IntoResponse {
    // JWT tokens are stateless, so logout is a client-side operation
    // The client should discard the token
    (
        StatusCode::OK,
        Json(json!({ "message": "Logged out successfully" })),
    )
}

/// Get current user info.
pub async fn me(user: AuthUser) -> impl IntoResponse {
    (StatusCode::OK, Json(user))
}

/// Refresh token endpoint.
pub async fn refresh(State(_state): State<AuthApiState>) -> impl IntoResponse {
    // For simplicity, this would need the old token passed in
    // In production, you'd implement proper refresh token flow
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({ "error": "Not implemented" })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::LoginRequest;
    use crate::auth::models::Role;
    use axum::extract::State;

    #[tokio::test]
    async fn login_with_default_user_returns_token() {
        let state = AuthApiState::new(JwtConfig::default());
        let query = LoginQuery { use_api_key: false };
        let payload = LoginRequest {
            username: "admin".to_string(),
            password: "admin".to_string(),
        };
        let result = login(State(state), Query(query), Json(payload)).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.0.access_token.is_empty());
        assert_eq!(response.0.token_type, "Bearer");
    }

    #[tokio::test]
    async fn login_with_wrong_password_fails() {
        let state = AuthApiState::new(JwtConfig::default());
        let query = LoginQuery { use_api_key: false };
        let payload = LoginRequest {
            username: "admin".to_string(),
            password: "wrong".to_string(),
        };
        let result = login(State(state), Query(query), Json(payload)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn refresh_returns_not_implemented() {
        let state = AuthApiState::new(JwtConfig::default());
        let response = refresh(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn logout_returns_ok() {
        let user = AuthUser {
            user_id: "test-id".to_string(),
            username: "admin".to_string(),
            role: Role::Admin,
        };
        let response = logout(user).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn me_returns_user_info() {
        let user = AuthUser {
            user_id: "u1".to_string(),
            username: "testuser".to_string(),
            role: Role::User,
        };
        let response = me(user).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
