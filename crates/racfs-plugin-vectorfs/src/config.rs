//! Configuration for the VectorFS filesystem.

/// Configuration for the VectorFS filesystem.
///
/// Storage is either a local path or an S3 URI. For S3, use `s3://bucket/prefix`;
/// credentials and region come from the environment (e.g. `AWS_ACCESS_KEY_ID`,
/// `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`) or the default credential chain.
#[derive(Debug, Clone)]
pub struct VectorConfig {
    /// Storage URI: local path (e.g. `./vector_db`, `/path/to/db`) or S3 URI (`s3://bucket/prefix`).
    pub storage_uri: String,

    /// API URL for embeddings (optional).
    pub embedding_api: Option<String>,

    /// Table name for the LanceDB vector table.
    pub table_name: String,

    /// Vector dimension for embeddings.
    /// Default is 384 (common for sentence-transformers models).
    pub dimension: usize,
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            storage_uri: "./vector_db".to_string(),
            embedding_api: None,
            table_name: "vectors".to_string(),
            dimension: 384,
        }
    }
}
