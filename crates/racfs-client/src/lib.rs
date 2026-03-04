//! RACFS HTTP Client.
//!
//! Async client for the RACFS REST API. Use [`Client`] to talk to a running
//! RACFS server (e.g. `racfs-server`) or any compatible API.
//!
//! # Example
//!
//! ```no_run
//! use racfs_client::Client;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::new("http://127.0.0.1:8080");
//! let health = client.health().await?;
//! println!("{}", health.status);
//! client.mkdir("/memfs/mydir", Some(0o755)).await?;
//! client.create_file("/memfs/mydir/hello.txt").await?;
//! client.write_file("/memfs/mydir/hello.txt", "Hello", None).await?;
//! let content = client.read_file("/memfs/mydir/hello.txt").await?;
//! # Ok(())
//! # }
//! ```
//!
//! Optional JWT auth: call [`Client::login`] or [`Client::set_token`]; the client
//! then sends the `Authorization: Bearer <token>` header on subsequent requests.

pub mod client;
pub mod types;

pub use client::{Client, ClientBuilder, Error};
pub use types::*;
