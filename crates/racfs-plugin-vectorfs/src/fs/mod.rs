mod chmod;
mod dir;
mod read;
mod write;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use arrow_array::{
    Array, BinaryArray, FixedSizeListArray, Float32Array, Int64Array, RecordBatch,
    RecordBatchIterator, StringArray, UInt32Array,
};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use chrono::Utc;
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use parking_lot::RwLock;
use racfs_core::{
    error::FSError,
    filesystem::{FileSystem, WriteFS},
    metadata::FileMetadata,
};

use crate::config::VectorConfig;

/// Extended attributes map: path -> (name -> value).
pub(crate) type XattrMap = Arc<RwLock<HashMap<PathBuf, HashMap<String, Vec<u8>>>>>;

/// VectorFS filesystem implementation.
///
/// Stores documents and vector embeddings in LanceDB. Storage can be a local path
/// or S3 URI (`s3://bucket/prefix`). Use [`VectorFS::with_config_async`] to construct.
///
/// When [`VectorConfig::embedding_api`] is set, document and query text is sent
/// to that URL for embedding; otherwise a content-based deterministic vector is used.
pub struct VectorFS {
    /// Configuration for the filesystem.
    pub(crate) config: VectorConfig,

    /// Optional HTTP client for embedding API calls.
    embedding_client: Option<reqwest::Client>,

    /// LanceDB table for documents (path, data, vector, mode, created_at, modified_at).
    table: Arc<lancedb::Table>,

    /// Metadata cache for virtual paths (/index/*, /search/*).
    metadata: Arc<RwLock<HashMap<PathBuf, FileMetadata>>>,

    /// Query text per search result directory: key = query_id (e.g. "abc" for /search/abc/).
    search_queries: Arc<RwLock<HashMap<String, Vec<u8>>>>,

    /// Extended attributes for document paths.
    xattrs: XattrMap,
}

/// Arrow schema for the vectors table: path, data, vector, mode, created_at, modified_at.
pub(crate) fn vectors_schema(dimension: usize) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("path", DataType::Utf8, false),
        Field::new("data", DataType::Binary, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimension as i32,
            ),
            true,
        ),
        Field::new("mode", DataType::UInt32, false),
        Field::new("created_at", DataType::Int64, true),
        Field::new("modified_at", DataType::Int64, true),
    ]))
}

impl VectorFS {
    /// Create a new VectorFS with default configuration (async).
    /// Uses local path `./vector_db` by default.
    pub async fn new_async() -> Result<Self, FSError> {
        Self::with_config_async(VectorConfig::default()).await
    }

    /// Create a new VectorFS with the given configuration (async).
    /// Connects to LanceDB at `config.storage_uri` (local path or `s3://bucket/prefix`).
    pub async fn with_config_async(config: VectorConfig) -> Result<Self, FSError> {
        let embedding_client = config
            .embedding_api
            .as_ref()
            .map(|_| reqwest::Client::new());

        let conn = lancedb::connect(&config.storage_uri)
            .execute()
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB connect failed: {}", e),
            })?;

        let table = Self::open_or_create_table(&conn, &config).await?;
        let table = Arc::new(table);

        let metadata = Arc::new(RwLock::new(HashMap::new()));
        let search_queries = Arc::new(RwLock::new(HashMap::new()));
        let xattrs = Arc::new(RwLock::new(HashMap::new()));

        let fs = Self {
            config: config.clone(),
            embedding_client,
            table,
            metadata,
            search_queries,
            xattrs,
        };

        fs.init_virtual_paths();
        Ok(fs)
    }

    /// Create a new VectorFS with the given configuration (sync).
    /// **Deprecated**: Use [`VectorFS::with_config_async`] instead. This runs the async
    /// constructor on the current runtime and may panic if no runtime is available.
    #[deprecated(
        since = "0.2.0",
        note = "Use with_config_async and await from async context; LanceDB requires async init"
    )]
    pub fn with_config(config: VectorConfig) -> Self {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(Self::with_config_async(config))
        })
        .expect("VectorFS::with_config_async failed")
    }

    async fn open_or_create_table(
        conn: &lancedb::Connection,
        config: &VectorConfig,
    ) -> Result<lancedb::Table, FSError> {
        let names = conn
            .table_names()
            .execute()
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB list tables: {}", e),
            })?;

        if names.iter().any(|n| n.as_str() == config.table_name) {
            conn.open_table(&config.table_name)
                .execute()
                .await
                .map_err(|e| FSError::Io {
                    message: format!("LanceDB open_table: {}", e),
                })
        } else {
            let schema = vectors_schema(config.dimension);
            let batch = RecordBatch::new_empty(schema.clone());
            let stream = RecordBatchIterator::new(vec![batch].into_iter().map(Ok), schema.clone());
            conn.create_table(&config.table_name, Box::new(stream))
                .execute()
                .await
                .map_err(|e| FSError::Io {
                    message: format!("LanceDB create_table: {}", e),
                })
        }
    }

    /// Initialize the virtual directory structure.
    fn init_virtual_paths(&self) {
        let mut meta = self.metadata.write();

        meta.insert(
            PathBuf::from("/"),
            FileMetadata::directory(PathBuf::from("/")),
        );
        meta.insert(
            PathBuf::from("/documents"),
            FileMetadata::directory(PathBuf::from("/documents")),
        );
        meta.insert(
            PathBuf::from("/index"),
            FileMetadata::directory(PathBuf::from("/index")),
        );
        meta.insert(
            PathBuf::from("/index/count"),
            FileMetadata::file(PathBuf::from("/index/count"), 0),
        );
        let dim_str = self.config.dimension.to_string();
        meta.insert(
            PathBuf::from("/index/dimension"),
            FileMetadata::file(PathBuf::from("/index/dimension"), dim_str.len() as u64),
        );
        meta.insert(
            PathBuf::from("/index/status"),
            FileMetadata::file(PathBuf::from("/index/status"), 5),
        );
        meta.insert(
            PathBuf::from("/search"),
            FileMetadata::directory(PathBuf::from("/search")),
        );
    }

    /// Check if a path is a virtual path (read-only index/search files).
    pub(crate) fn is_virtual_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.starts_with("/index/") || path_str.starts_with("/search/")
    }

    /// Get the document count from LanceDB.
    pub(crate) async fn document_count(&self) -> Result<usize, FSError> {
        self.table.count_rows(None).await.map_err(|e| FSError::Io {
            message: format!("LanceDB count_rows: {}", e),
        })
    }

    /// Get the vector dimension.
    pub(crate) fn dimension(&self) -> usize {
        self.config.dimension
    }

    /// Fetch a single document row by path. Returns (data, vector, mode, created_at, modified_at).
    pub(crate) async fn get_document(
        &self,
        path: &Path,
    ) -> Result<Option<(Vec<u8>, Vec<f32>, u32, Option<i64>, Option<i64>)>, FSError> {
        let path_str = path.to_string_lossy();
        let predicate = format!("path = '{}'", escape_sql_string(path_str.as_ref()));

        let stream = self
            .table
            .query()
            .only_if(&predicate)
            .limit(1)
            .execute()
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB query by path: {}", e),
            })?;
        let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| FSError::Io {
            message: format!("LanceDB query collect: {}", e),
        })?;

        let batch = match batches.first() {
            Some(b) if b.num_rows() > 0 => b,
            _ => return Ok(None),
        };

        let path_col = batch
            .column_by_name("path")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| FSError::Io {
                message: "LanceDB: path column".to_string(),
            })?;
        let data_col = batch
            .column_by_name("data")
            .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
            .ok_or_else(|| FSError::Io {
                message: "LanceDB: data column".to_string(),
            })?;
        let vector_col = batch
            .column_by_name("vector")
            .and_then(|c| c.as_any().downcast_ref::<FixedSizeListArray>())
            .ok_or_else(|| FSError::Io {
                message: "LanceDB: vector column".to_string(),
            })?;
        let mode_col = batch
            .column_by_name("mode")
            .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
            .ok_or_else(|| FSError::Io {
                message: "LanceDB: mode column".to_string(),
            })?;
        let created_col = batch
            .column_by_name("created_at")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
        let modified_col = batch
            .column_by_name("modified_at")
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>());

        if path_col.is_null(0) || data_col.is_null(0) || mode_col.is_null(0) {
            return Ok(None);
        }

        let data: Vec<u8> = data_col.value(0).into();
        let mode = mode_col.value(0);
        let created_at =
            created_col.and_then(|c| if c.is_null(0) { None } else { Some(c.value(0)) });
        let modified_at =
            modified_col.and_then(|c| if c.is_null(0) { None } else { Some(c.value(0)) });

        let vector = if vector_col.is_null(0) {
            vec![]
        } else {
            let inner = vector_col.value(0);
            let inner_f32 = inner
                .as_any()
                .downcast_ref::<Float32Array>()
                .ok_or_else(|| FSError::Io {
                    message: "LanceDB: vector inner Float32".to_string(),
                })?;
            (0..inner_f32.len())
                .map(|i| inner_f32.value(i))
                .collect::<Vec<f32>>()
        };

        Ok(Some((data, vector, mode, created_at, modified_at)))
    }

    /// Search for similar documents using LanceDB vector search.
    /// Returns up to `limit` results sorted by similarity (highest first).
    /// Call this directly from Rust when you need a custom limit without using the filesystem interface.
    pub async fn search(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<(PathBuf, f32)>, FSError> {
        if query.len() != self.config.dimension {
            return Err(FSError::InvalidInput {
                message: format!(
                    "query dimension {} != config dimension {}",
                    query.len(),
                    self.config.dimension
                ),
            });
        }

        let stream = self
            .table
            .query()
            .nearest_to(query)
            .map_err(|e| FSError::Io {
                message: format!("LanceDB nearest_to: {}", e),
            })?
            .limit(limit)
            .execute()
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB vector search: {}", e),
            })?;

        let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| FSError::Io {
            message: format!("LanceDB search collect: {}", e),
        })?;

        let mut results = Vec::new();
        for batch in batches {
            let path_col = batch
                .column_by_name("path")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dist_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

            let path_col = match path_col {
                Some(c) => c,
                None => continue,
            };

            for i in 0..batch.num_rows() {
                if path_col.is_null(i) {
                    continue;
                }
                let path_str = path_col.value(i);
                let path = PathBuf::from(path_str);
                // LanceDB returns L2 distance; we use negative so "higher is better" for similarity
                let score = dist_col
                    .and_then(|d| {
                        if d.is_null(i) {
                            None
                        } else {
                            Some(-d.value(i))
                        }
                    })
                    .unwrap_or(0.0);
                results.push((path, score));
            }
        }
        Ok(results)
    }

    /// Upsert a document into LanceDB (merge_insert on path).
    pub(crate) async fn persist_document(
        &self,
        path_str: &str,
        data: &[u8],
        vector: &[f32],
        mode: u32,
        created_at: Option<i64>,
        modified_at: Option<i64>,
    ) -> Result<(), FSError> {
        let schema = vectors_schema(self.config.dimension);
        let path_arr = StringArray::from(vec![path_str]);
        let data_arr = BinaryArray::from_iter_values([data]);
        let vector_inner: Vec<Option<f32>> = vector.iter().copied().map(Some).collect();
        let vector_arr =
            FixedSizeListArray::from_iter_primitive::<arrow_array::types::Float32Type, _, _>(
                std::iter::once(Some(vector_inner)),
                self.config.dimension as i32,
            );
        let mode_arr = UInt32Array::from(vec![mode]);
        let created_arr = Int64Array::from(vec![created_at.unwrap_or(0)]);
        let modified_arr = Int64Array::from(vec![modified_at.unwrap_or(0)]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(path_arr),
                Arc::new(data_arr),
                Arc::new(vector_arr),
                Arc::new(mode_arr),
                Arc::new(created_arr),
                Arc::new(modified_arr),
            ],
        )
        .map_err(|e| FSError::Io {
            message: format!("RecordBatch: {}", e),
        })?;

        let stream = RecordBatchIterator::new(vec![batch].into_iter().map(Ok), schema.clone());
        let mut merge = self.table.merge_insert(&["path"]);
        merge
            .when_matched_update_all(None)
            .when_not_matched_insert_all();
        merge
            .execute(Box::new(stream))
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB merge_insert: {}", e),
            })?;
        Ok(())
    }

    /// Delete a document by path from LanceDB.
    pub(crate) async fn delete_from_db(&self, path_str: &str) -> Result<(), FSError> {
        let predicate = format!("path = '{}'", escape_sql_string(path_str));
        self.table
            .delete(&predicate)
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB delete: {}", e),
            })?;
        Ok(())
    }

    /// Generate a deterministic vector from a seed (for content-based "embedding" without an API).
    fn seed_to_vector(&self, seed: u64) -> Vec<f32> {
        let mut lcg = seed;
        let mut vector = Vec::with_capacity(self.config.dimension);
        for _ in 0..self.config.dimension {
            lcg = lcg.wrapping_mul(1103515245).wrapping_add(12345);
            let value = ((lcg >> 16) & 0x7fff) as f32 / 32768.0;
            vector.push(value);
        }
        let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for v in &mut vector {
                *v /= magnitude;
            }
        }
        vector
    }

    /// Derive a vector from content bytes (hash to seed, then seed_to_vector).
    fn content_to_vector(&self, content: &[u8]) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        self.seed_to_vector(hasher.finish())
    }

    /// Get embedding for content: use external API if configured, otherwise content-based vector.
    pub(crate) async fn embed_content(&self, content: &[u8]) -> Result<Vec<f32>, FSError> {
        let Some(ref client) = self.embedding_client else {
            return Ok(self.content_to_vector(content));
        };
        let url = self
            .config
            .embedding_api
            .as_ref()
            .ok_or_else(|| FSError::InvalidInput {
                message: "embedding_api not set".to_string(),
            })?;
        let text = std::str::from_utf8(content).map_err(|_| FSError::InvalidInput {
            message: "Content must be valid UTF-8 when using embedding API".to_string(),
        })?;
        let endpoint = format!("{}/embed", url.trim_end_matches('/'));
        let res = client
            .post(&endpoint)
            .body(text.to_string())
            .header("Content-Type", "text/plain; charset=utf-8")
            .send()
            .await
            .map_err(|e| FSError::InvalidInput {
                message: format!("Embedding API request failed: {}", e),
            })?;
        let status = res.status();
        let body = res.bytes().await.map_err(|e| FSError::InvalidInput {
            message: format!("Embedding API response read failed: {}", e),
        })?;
        if !status.is_success() {
            return Err(FSError::InvalidInput {
                message: format!(
                    "Embedding API returned {}: {}",
                    status,
                    String::from_utf8_lossy(&body)
                ),
            });
        }
        let vec: Vec<f64> = serde_json::from_slice(&body).map_err(|e| FSError::InvalidInput {
            message: format!("Embedding API response not JSON array: {}", e),
        })?;
        if vec.len() != self.config.dimension {
            return Err(FSError::InvalidInput {
                message: format!(
                    "Embedding API returned dimension {} but expected {}",
                    vec.len(),
                    self.config.dimension
                ),
            });
        }
        Ok(vec.into_iter().map(|x| x as f32).collect())
    }

    /// Parse pre-computed vector from xattr value (JSON array of numbers).
    pub(crate) fn parse_vector_xattr(value: &[u8], dimension: usize) -> Result<Vec<f32>, FSError> {
        let vec: Vec<f64> = serde_json::from_slice(value).map_err(|e| FSError::InvalidInput {
            message: format!("racfs.vector xattr must be JSON array of numbers: {}", e),
        })?;
        if vec.len() != dimension {
            return Err(FSError::InvalidInput {
                message: format!(
                    "racfs.vector dimension {} does not match config dimension {}",
                    vec.len(),
                    dimension
                ),
            });
        }
        Ok(vec.into_iter().map(|x| x as f32).collect())
    }

    /// List all document paths from LanceDB (for read_dir /documents).
    pub(crate) async fn list_document_paths(&self) -> Result<Vec<PathBuf>, FSError> {
        let stream = self
            .table
            .query()
            .execute()
            .await
            .map_err(|e| FSError::Io {
                message: format!("LanceDB list paths: {}", e),
            })?;
        let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| FSError::Io {
            message: format!("LanceDB list collect: {}", e),
        })?;
        let mut paths = Vec::new();
        for batch in batches {
            let path_col = batch
                .column_by_name("path")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            if let Some(col) = path_col {
                for i in 0..col.len() {
                    if !col.is_null(i) {
                        paths.push(PathBuf::from(col.value(i)));
                    }
                }
            }
        }
        Ok(paths)
    }
}

fn escape_sql_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "''")
}

impl Default for VectorFS {
    fn default() -> Self {
        #[allow(deprecated)]
        Self::with_config(VectorConfig::default())
    }
}

#[async_trait]
impl FileSystem for VectorFS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        if self.is_virtual_path(path) {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }
        let path_str = path.to_string_lossy();
        if !path_str.starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "Can only truncate files in /documents/".to_string(),
            });
        }

        let Some((mut data, vector, mode, created_at, _modified_at)) =
            self.get_document(path).await?
        else {
            return Err(FSError::NotFound {
                path: path.to_path_buf(),
            });
        };

        let current_size = data.len() as u64;
        if size < current_size {
            data.truncate(size as usize);
        } else if size > current_size {
            data.resize(size as usize, 0);
        }

        let modified = Utc::now().timestamp_millis();
        self.persist_document(&path_str, &data, &vector, mode, created_at, Some(modified))
            .await?;

        if let Some(meta) = self.metadata.write().get_mut(path) {
            meta.size = size;
            meta.modified = chrono::DateTime::from_timestamp_millis(modified);
        }
        Ok(())
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        if self.is_virtual_path(path) && path.to_string_lossy().starts_with("/index/") {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }
        let path_str = path.to_string_lossy();
        if !path_str.starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "Can only touch files in /documents/".to_string(),
            });
        }

        if let Some((data, vector, mode, created_at, _)) = self.get_document(path).await? {
            let modified = Utc::now().timestamp_millis();
            self.persist_document(&path_str, &data, &vector, mode, created_at, Some(modified))
                .await?;
            if let Some(m) = self.metadata.write().get_mut(path) {
                m.accessed = chrono::DateTime::from_timestamp_millis(modified);
                m.modified = m.accessed;
            }
            return Ok(());
        }

        self.create(path).await?;
        if let Some(m) = self.metadata.write().get_mut(path) {
            let now = Utc::now();
            m.accessed = Some(now);
            m.modified = Some(now);
        }
        Ok(())
    }

    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Vec<u8>, FSError> {
        if !path.to_string_lossy().starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "xattr only supported on document paths".to_string(),
            });
        }
        self.get_document(path)
            .await?
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })?;
        let xattrs = self.xattrs.read();
        let per_path = xattrs.get(path).ok_or_else(|| FSError::InvalidInput {
            message: "extended attribute not found".to_string(),
        })?;
        per_path
            .get(name)
            .cloned()
            .ok_or_else(|| FSError::InvalidInput {
                message: "extended attribute not found".to_string(),
            })
    }

    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<(), FSError> {
        if !path.to_string_lossy().starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "xattr only supported on document paths".to_string(),
            });
        }
        self.get_document(path)
            .await?
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })?;
        self.xattrs
            .write()
            .entry(path.to_path_buf())
            .or_default()
            .insert(name.to_string(), value.to_vec());
        Ok(())
    }

    async fn remove_xattr(&self, path: &Path, name: &str) -> Result<(), FSError> {
        if !path.to_string_lossy().starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "xattr only supported on document paths".to_string(),
            });
        }
        self.get_document(path)
            .await?
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })?;
        let mut xattrs = self.xattrs.write();
        let per_path = xattrs.get_mut(path).ok_or_else(|| FSError::InvalidInput {
            message: "extended attribute not found".to_string(),
        })?;
        per_path.remove(name).ok_or_else(|| FSError::InvalidInput {
            message: "extended attribute not found".to_string(),
        })?;
        Ok(())
    }

    async fn list_xattr(&self, path: &Path) -> Result<Vec<String>, FSError> {
        if !path.to_string_lossy().starts_with("/documents/") {
            return Err(FSError::NotSupported {
                message: "xattr only supported on document paths".to_string(),
            });
        }
        self.get_document(path)
            .await?
            .ok_or_else(|| FSError::NotFound {
                path: path.to_path_buf(),
            })?;
        let xattrs = self.xattrs.read();
        let names: Vec<String> = xattrs
            .get(path)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        Ok(names)
    }
}
