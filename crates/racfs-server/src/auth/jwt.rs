//! JWT authentication implementation.

use std::sync::Arc;

use bcrypt::{hash, verify};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::AuthError;
use super::models::{LoginRequest, LoginResponse, Role, User};

/// JWT authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// Secret key for signing tokens
    pub secret: String,
    /// Token expiration in seconds (default: 1 hour)
    #[serde(default = "default_expiration")]
    pub expiration: u64,
    /// Token issuer
    #[serde(default = "default_issuer")]
    pub issuer: String,
    /// JWT algorithm
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
}

fn default_expiration() -> u64 {
    3600 // 1 hour
}

fn default_issuer() -> String {
    "racfs".to_string()
}

fn default_algorithm() -> String {
    "HS256".to_string()
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: Uuid::new_v4().to_string(), // Generate random secret for dev
            expiration: default_expiration(),
            issuer: default_issuer(),
            algorithm: default_algorithm(),
        }
    }
}

/// JWT authentication handler.
#[derive(Clone)]
pub struct JwtAuth {
    config: JwtConfig,
    /// In-memory user store (for development/demo)
    users: Arc<RwLock<Vec<User>>>,
    algorithm: Algorithm,
    encoding_key: EncodingKey,
}

impl JwtAuth {
    /// Create a new JwtAuth instance.
    pub fn new(config: JwtConfig) -> Self {
        let algorithm = match config.algorithm.to_uppercase().as_str() {
            "HS256" => Algorithm::HS256,
            "HS384" => Algorithm::HS384,
            "HS512" => Algorithm::HS512,
            _ => Algorithm::HS256,
        };

        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());

        let mut users = Vec::new();

        // Add default admin user for development
        let admin_user = User {
            id: Uuid::new_v4().to_string(),
            username: "admin".to_string(),
            password_hash: hash("admin", 10).unwrap_or_default(),
            role: Role::Admin,
        };
        users.push(admin_user);

        // Add default user for development
        let default_user = User {
            id: Uuid::new_v4().to_string(),
            username: "user".to_string(),
            password_hash: hash("password", 10).unwrap_or_default(),
            role: Role::User,
        };
        users.push(default_user);

        Self {
            config,
            users: Arc::new(RwLock::new(users)),
            algorithm,
            encoding_key,
        }
    }

    /// Authenticate user with username and password.
    pub fn authenticate(&self, request: &LoginRequest) -> Result<LoginResponse, AuthError> {
        let users = self.users.read();
        let user = users
            .iter()
            .find(|u| u.username == request.username)
            .ok_or(AuthError::InvalidCredentials)?;

        // Verify password
        let valid = verify(&request.password, &user.password_hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

        if !valid {
            return Err(AuthError::InvalidCredentials);
        }

        // Generate token
        self.generate_token(user)
    }

    /// Generate JWT token for user.
    pub fn generate_token(&self, user: &User) -> Result<LoginResponse, AuthError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| AuthError::Internal {
                source: Box::new(e),
            })?
            .as_secs();

        let claims = super::models::Claims {
            sub: user.id.clone(),
            username: user.username.clone(),
            role: user.role,
            exp: now + self.config.expiration,
            iat: now,
        };

        let token = encode(&Header::new(self.algorithm), &claims, &self.encoding_key)
            .map_err(|e| AuthError::TokenValidation { source: e })?;

        Ok(LoginResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.expiration,
            refresh_token: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_auth_default_config() {
        let config = JwtConfig::default();
        assert_eq!(config.expiration, 3600);
        assert_eq!(config.issuer, "racfs");
        assert_eq!(config.algorithm, "HS256");
    }

    #[test]
    fn test_authenticate_default_user() {
        let auth = JwtAuth::new(JwtConfig::default());
        let request = LoginRequest {
            username: "admin".to_string(),
            password: "admin".to_string(),
        };

        let result = auth.authenticate(&request);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.token_type, "Bearer");
    }

    #[test]
    fn test_invalid_credentials() {
        let auth = JwtAuth::new(JwtConfig::default());
        let request = LoginRequest {
            username: "admin".to_string(),
            password: "wrong_password".to_string(),
        };

        let result = auth.authenticate(&request);
        assert!(result.is_err());
    }
}
