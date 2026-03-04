//! S3-compatible object storage filesystem plugin.

mod config;
mod fs;

#[cfg(test)]
mod tests;

pub use config::S3Config;
pub use fs::S3FS;
