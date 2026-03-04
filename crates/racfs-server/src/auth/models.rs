//! Authentication data models.

use serde::{Deserialize, Serialize};

/// JWT claims for authenticated users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Username
    pub username: String,
    /// Role
    pub role: Role,
    /// Token expiration timestamp
    pub exp: u64,
    /// Issued at timestamp
    pub iat: u64,
}

/// User roles for RBAC.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Admin role with full access
    Admin,
    /// Regular user with read/write access
    #[default]
    User,
    /// Read-only user
    Readonly,
}

/// User account for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique user ID
    pub id: String,
    /// Username
    pub username: String,
    /// Hashed password
    pub password_hash: String,
    /// User role
    pub role: Role,
}

/// Login request payload.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Username
    pub username: String,
    /// Password
    pub password: String,
}

/// Login response.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// JWT access token
    pub access_token: String,
    /// Token type (Bearer)
    pub token_type: String,
    /// Token expiration in seconds
    pub expires_in: u64,
    /// Refresh token
    pub refresh_token: Option<String>,
}

/// API key authentication data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key ID
    pub id: String,
    /// Key value (hashed)
    pub key_hash: String,
    /// Associated user ID
    pub user_id: String,
    /// Key name/description
    pub name: String,
    /// Whether the key is active
    pub active: bool,
}
