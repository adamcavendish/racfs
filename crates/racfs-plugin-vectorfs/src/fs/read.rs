use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::VectorFS;

/// Default number of results returned when reading matches.txt if no limit is specified in query.txt.
pub const DEFAULT_SEARCH_LIMIT: usize = 10;

/// Maximum allowed search limit (when parsing from query.txt JSON) to avoid unbounded results.
pub const MAX_SEARCH_LIMIT: usize = 1000;

/// Parse query.txt content: optional JSON `{"query": "...", "limit": N}` or plain text.
/// Returns (query bytes for embedding, limit to use for search).
fn parse_query_and_limit(query_bytes: &[u8]) -> (Vec<u8>, usize) {
    let trimmed = trim_whitespace(query_bytes);
    if trimmed.is_empty() {
        return (Vec::new(), DEFAULT_SEARCH_LIMIT);
    }
    match serde_json::from_slice::<serde_json::Value>(trimmed) {
        Ok(serde_json::Value::Object(obj)) => {
            let query = obj
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| s.as_bytes().to_vec());
            let limit = obj
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(DEFAULT_SEARCH_LIMIT);
            let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
            let query_bytes = query.unwrap_or_else(|| query_bytes.to_vec());
            (query_bytes, limit)
        }
        _ => (query_bytes.to_vec(), DEFAULT_SEARCH_LIMIT),
    }
}

fn trim_whitespace(b: &[u8]) -> &[u8] {
    let start = b
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    let end = b
        .iter()
        .rposition(|c| !c.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    &b[start..end]
}

#[async_trait]
impl ReadFS for VectorFS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        let path_str = path.to_string_lossy();

        // Handle virtual files
        if path_str == "/index/count" {
            let count = self.document_count().await?;
            let data = count.to_string().into_bytes();
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset + size).min(data.len() as i64) as usize
            };
            return if start >= data.len() {
                Ok(Vec::new())
            } else {
                Ok(data[start..end].to_vec())
            };
        }

        if path_str == "/index/status" {
            let data = b"ready".to_vec();
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset + size).min(data.len() as i64) as usize
            };
            return if start >= data.len() {
                Ok(Vec::new())
            } else {
                Ok(data[start..end].to_vec())
            };
        }

        if path_str == "/index/dimension" {
            let dim = self.dimension();
            let data = dim.to_string().into_bytes();
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset + size).min(data.len() as i64) as usize
            };
            return if start >= data.len() {
                Ok(Vec::new())
            } else {
                Ok(data[start..end].to_vec())
            };
        }

        // Handle documents
        if path_str.starts_with("/documents/") {
            let Some((data, ..)) = self.get_document(path).await? else {
                return Err(FSError::NotFound {
                    path: path.to_path_buf(),
                });
            };

            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset + size).min(data.len() as i64) as usize
            };

            if start >= data.len() {
                return Ok(Vec::new());
            }

            return Ok(data[start..end].to_vec());
        }

        // Handle /search/{id}/query.txt
        if path_str.contains("/search/") && path_str.ends_with("/query.txt") {
            let query_id = path_str
                .trim_start_matches("/search/")
                .trim_end_matches("/query.txt")
                .trim_end_matches('/');
            let queries = self.search_queries.read();
            let data = queries.get(query_id).cloned().unwrap_or_default();
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset as usize + size as usize).min(data.len())
            };
            return if start >= data.len() {
                Ok(Vec::new())
            } else {
                Ok(data[start..end].to_vec())
            };
        }

        // Handle /search/{id}/matches.txt — run search and return JSON
        if path_str.contains("/search/") && path_str.ends_with("/matches.txt") {
            let query_id = path_str
                .trim_start_matches("/search/")
                .trim_end_matches("/matches.txt")
                .trim_end_matches('/');
            let query_bytes = self
                .search_queries
                .read()
                .get(query_id)
                .cloned()
                .unwrap_or_default();
            let (query_for_embed, limit) = parse_query_and_limit(&query_bytes);
            let query_vector = self.embed_content(&query_for_embed).await?;
            let results = self.search(&query_vector, limit).await?;
            #[derive(serde::Serialize)]
            struct Match {
                path: String,
                score: f32,
            }
            let matches: Vec<Match> = results
                .into_iter()
                .map(|(p, s)| Match {
                    path: p.to_string_lossy().to_string(),
                    score: s,
                })
                .collect();
            let data = serde_json::to_vec(&matches).unwrap_or_else(|_| b"[]".to_vec());
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                data.len()
            } else {
                (offset as usize + size as usize).min(data.len())
            };
            return if start >= data.len() {
                Ok(Vec::new())
            } else {
                Ok(data[start..end].to_vec())
            };
        }

        Err(FSError::NotFound {
            path: path.to_path_buf(),
        })
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        let path_str = path.to_string_lossy();

        if path_str == "/index/count" {
            let mut meta =
                self.metadata
                    .read()
                    .get(path)
                    .cloned()
                    .ok_or_else(|| FSError::NotFound {
                        path: path.to_path_buf(),
                    })?;
            meta.size = self.document_count().await? as u64;
            return Ok(meta);
        }

        if path_str == "/index/status" || path_str == "/index/dimension" {
            return self
                .metadata
                .read()
                .get(path)
                .cloned()
                .ok_or_else(|| FSError::NotFound {
                    path: path.to_path_buf(),
                });
        }

        if path_str.contains("/search/") && path_str.ends_with("/query.txt") {
            let query_id = path_str
                .trim_start_matches("/search/")
                .trim_end_matches("/query.txt")
                .trim_end_matches('/');
            if let Some(meta) = self.metadata.read().get(path).cloned() {
                let len = self
                    .search_queries
                    .read()
                    .get(query_id)
                    .map(|q| q.len())
                    .unwrap_or(0);
                let mut m = meta;
                m.size = len as u64;
                return Ok(m);
            }
        }

        if path_str.contains("/search/") && path_str.ends_with("/matches.txt")
            && let Some(mut meta) = self.metadata.read().get(path).cloned()
        {
            meta.size = 0; // dynamic content, actual size determined on read()
            return Ok(meta);
        }

        if path_str.starts_with("/documents/")
            && let Some((data, _, mode, created_at, modified_at)) = self.get_document(path).await?
        {
            let created = created_at.and_then(chrono::DateTime::from_timestamp_millis);
            let modified = modified_at.and_then(chrono::DateTime::from_timestamp_millis);
            let meta = FileMetadata {
                path: path.to_path_buf(),
                size: data.len() as u64,
                mode,
                created,
                modified,
                accessed: modified,
                is_symlink: false,
                symlink_target: None,
            };
            return Ok(meta);
        }

        self.metadata
            .read()
            .get(path)
            .cloned()
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })
    }
}
