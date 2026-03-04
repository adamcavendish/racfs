//! HTTP Error Response types for standardized error handling

mod error_code;
mod error_response;

pub use error_code::ErrorCode;
pub use error_response::HttpErrorResponse;
