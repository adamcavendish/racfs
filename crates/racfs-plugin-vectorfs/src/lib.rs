//! Vector database filesystem (vectorfs) plugin.
//!
//! This plugin provides a filesystem interface for storing and searching documents
//! using vector embeddings via **LanceDB**. Storage can be a local path or S3
//! (`s3://bucket/prefix`). Use [`VectorFS::with_config_async`] to construct.
//!
//! ```text
//! /
//! |-- documents/           # Store documents as files
//! |   |-- {id}.txt        # Write document, auto-indexes
//! |-- index/              # Read-only index info
//! |   |-- count           # Number of indexed documents
//! |   |-- status          # "ready" or "indexing"
//! |-- search/             # Search results (created on demand)
//!     |-- {query_id}/
//!         |-- query.txt   # The search query (write to set, read to get)
//!         |-- matches.txt # Ranked results as JSON (read runs search)
//! ```
//!
//! # Configuration and limits
//!
//! - **Dimension:** [`VectorConfig::dimension`] must match the output size of your embedding
//!   model (e.g. 384 for many sentence-transformers, 1536 for OpenAI text-embedding-3-small).
//!   Set via server TOML `dimension` under the vectorfs mount or when constructing the config.
//! - **Table name:** Configurable via TOML `table_name` for multi-tenant or per-agent tables.
//! - **Search limit:** When reading `matches.txt`, the limit is taken from `query.txt`:
//!   either plain text (default limit, e.g. 10) or JSON `{"query": "text", "limit": N}` with
//!   `limit` clamped to 1–1000. You can also call [`VectorFS::search`] from Rust with a custom limit.
//!
//! # Extended attributes (xattrs)
//!
//! Extended attributes (e.g. `racfs.vector` for a pre-computed vector) are stored **in memory
//! only** and are **not** persisted to LanceDB. They are lost on server restart. Vectors and
//! document content in LanceDB persist. For the typical use case (set xattr, write document,
//! xattr consumed at write time) this is acceptable.
//!
//! # stat() on matches.txt
//!
//! `stat()` on `/search/{id}/matches.txt` returns size 0 (the /proc convention for dynamic
//! pseudo-files). The actual content is generated on `read()`. This avoids running the full
//! vector search for metadata-only operations (e.g. FUSE getattr, ls -l).

mod config;
mod fs;

#[cfg(test)]
mod tests;

pub use config::VectorConfig;
pub use fs::VectorFS;
