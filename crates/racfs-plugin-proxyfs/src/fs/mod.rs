//! Proxy filesystem implementation.

mod chmod;
mod dir;
mod read;
mod write;

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use racfs_core::filesystem::FileSystem;

/// Proxy filesystem that forwards to a remote server.
pub struct ProxyFS {
    pub(crate) base_url: String,
    pub(crate) client: Client,
}

#[derive(Debug, Serialize)]
pub(crate) struct FileQuery {
    pub(crate) path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct WriteRequest {
    pub(crate) path: String,
    pub(crate) data: String,
    pub(crate) offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RenameRequest {
    pub(crate) old_path: String,
    pub(crate) new_path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChmodRequest {
    pub(crate) path: String,
    pub(crate) mode: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateDirectoryRequest {
    pub(crate) path: String,
    pub(crate) perm: Option<u32>,
}

impl ProxyFS {
    /// Create a new proxy filesystem.
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl FileSystem for ProxyFS {}
