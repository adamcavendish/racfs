//! Authentication module for RACFS server.
//!
//! Provides JWT-based authentication and API key support.

pub mod api_key;
pub mod error;
pub mod jwt;
pub mod middleware;
pub mod models;

pub use api_key::ApiKeyAuth;
pub use error::AuthError;
pub use jwt::{JwtAuth, JwtConfig};
pub use middleware::AuthUser;
