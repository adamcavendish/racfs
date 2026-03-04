//! API key authentication implementation.

use std::sync::Arc;

use bcrypt::verify;
use parking_lot::RwLock;
use uuid::Uuid;

use super::error::AuthError;
use super::models::{ApiKey, Role, User};

/// API key authentication handler.
#[derive(Clone)]
pub struct ApiKeyAuth {
    /// In-memory API key store
    keys: Arc<RwLock<Vec<ApiKey>>>,
    /// In-memory user store (references)
    users: Arc<RwLock<Vec<User>>>,
}

impl ApiKeyAuth {
    /// Create a new ApiKeyAuth instance.
    pub fn new() -> Self {
        // Add default user
        let users = vec![User {
            id: "default-user".to_string(),
            username: "api_user".to_string(),
            password_hash: String::new(),
            role: Role::User,
        }];

        // Add default API key for development
        let default_key = ApiKey {
            id: Uuid::new_v4().to_string(),
            key_hash: bcrypt::hash("racfs-api-key-12345", 10).unwrap_or_default(),
            user_id: "default-user".to_string(),
            name: "Default API Key".to_string(),
            active: true,
        };

        let keys = vec![default_key];

        Self {
            keys: Arc::new(RwLock::new(keys)),
            users: Arc::new(RwLock::new(users)),
        }
    }

    /// Validate an API key.
    pub fn validate_key(&self, api_key: &str) -> Result<User, AuthError> {
        let keys = self.keys.read();

        for key in keys.iter() {
            if !key.active {
                continue;
            }

            // Verify the key (compare hash)
            if verify(api_key, &key.key_hash).unwrap_or(false) {
                // Find the associated user
                let users = self.users.read();
                return users
                    .iter()
                    .find(|u| u.id == key.user_id)
                    .cloned()
                    .ok_or(AuthError::UserNotFound);
            }
        }

        Err(AuthError::InvalidApiKey)
    }
}

impl Default for ApiKeyAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_default_key() {
        let auth = ApiKeyAuth::new();
        let result = auth.validate_key("racfs-api-key-12345");
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_key() {
        let auth = ApiKeyAuth::new();
        let result = auth.validate_key("invalid-key");
        assert!(result.is_err());
    }
}
