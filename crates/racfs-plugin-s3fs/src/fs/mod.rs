//! S3 filesystem implementation using the AWS SDK for Rust (aws_sdk_s3).
mod chmod;
mod dir;
mod read;
mod write;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_types::region::Region;
use racfs_core::{
    error::FSError,
    filesystem::{FileSystem, ReadFS, WriteFS},
    metadata::FileMetadata,
};

use crate::config::S3Config;

/// S3 filesystem implementation.
///
/// Provides a virtual filesystem backed by S3 (or S3-compatible storage) using
/// the official AWS SDK for Rust (`aws_sdk_s3`). Required dependency.
pub struct S3FS {
    pub(crate) config: S3Config,
    client: aws_sdk_s3::Client,
}

impl S3FS {
    /// Create a new S3 filesystem instance using the given config.
    /// Builds an `aws_sdk_s3::Client` from config (region, endpoint, credentials).
    pub fn new(config: S3Config) -> Result<Self, FSError> {
        let client = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(Self::build_client(&config))
        })?;
        Ok(Self { config, client })
    }

    async fn build_client(config: &S3Config) -> Result<aws_sdk_s3::Client, FSError> {
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(config.region.clone()));

        if let Some(ep) = &config.endpoint {
            loader = loader.endpoint_url(ep);
        }

        if !config.access_key.is_empty() && !config.secret_key.is_empty() {
            let creds = Credentials::new(
                config.access_key.clone(),
                config.secret_key.clone(),
                None,
                None,
                "racfs-s3fs",
            );
            loader = loader.credentials_provider(creds);
        }

        let sdk_config = loader.load().await;
        let client = aws_sdk_s3::Client::new(&sdk_config);
        Ok(client)
    }

    /// Validate the configuration.
    pub(crate) fn validate_config(&self) -> Result<(), FSError> {
        if self.config.bucket.is_empty() {
            return Err(FSError::InvalidInput {
                message: "S3 bucket name cannot be empty".to_string(),
            });
        }
        if self.config.region.is_empty() {
            return Err(FSError::InvalidInput {
                message: "S3 region cannot be empty".to_string(),
            });
        }
        if self.config.access_key.is_empty() {
            return Err(FSError::InvalidInput {
                message: "S3 access key cannot be empty".to_string(),
            });
        }
        if self.config.secret_key.is_empty() {
            return Err(FSError::InvalidInput {
                message: "S3 secret key cannot be empty".to_string(),
            });
        }
        Ok(())
    }

    /// Convert S3 key to virtual path.
    pub(crate) fn key_to_path(&self, key: &str) -> PathBuf {
        if key.is_empty() || key == "/" {
            PathBuf::from("/")
        } else {
            PathBuf::from("/").join(key)
        }
    }

    /// Convert virtual path to S3 key.
    pub(crate) fn path_to_key(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        if path_str == "/" {
            String::new()
        } else {
            path_str.strip_prefix('/').unwrap_or(&path_str).to_string()
        }
    }

    fn sdk_error(e: impl std::fmt::Display) -> FSError {
        FSError::Io {
            message: e.to_string(),
        }
    }

    /// List objects with prefix and optional delimiter. Returns file metadata for Contents and CommonPrefixes.
    async fn list_objects_v2(
        &self,
        prefix: &str,
        delimiter: Option<&str>,
    ) -> Result<Vec<FileMetadata>, FSError> {
        self.validate_config()?;

        let mut results = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut list = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(prefix)
                .max_keys(1000);
            if let Some(d) = delimiter {
                list = list.delimiter(d);
            }
            if let Some(ref t) = continuation_token {
                list = list.continuation_token(t);
            }

            let resp = list.send().await.map_err(Self::sdk_error)?;

            for obj in resp.contents().iter() {
                let key = obj.key().unwrap_or_default();
                let size = obj.size().unwrap_or(0) as u64;
                let path = self.key_to_path(key);
                let mut meta = FileMetadata::file(path, size);
                if let Some(last_modified) = obj.last_modified() {
                    meta.modified = chrono::DateTime::from_timestamp(
                        last_modified.secs(),
                        last_modified.subsec_nanos(),
                    )
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                    meta.accessed = meta.modified;
                }
                results.push(meta);
            }

            for prefix_elem in resp.common_prefixes().iter() {
                if let Some(p) = prefix_elem.prefix() && !p.is_empty() {
                    results.push(FileMetadata::directory(self.key_to_path(p)));
                }
            }

            if !resp.is_truncated().unwrap_or(false) {
                break;
            }
            continuation_token = resp.next_continuation_token().map(String::from);
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Check if path exists (HEAD object or list prefix).
    async fn exists(&self, path: &Path) -> Result<bool, FSError> {
        let key = self.path_to_key(path);
        if key.is_empty() {
            return Ok(true);
        }

        let resp = self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await;

        match resp {
            Ok(_) => Ok(true),
            Err(e) => {
                let is_not_found = matches!(
                    &e,
                    SdkError::ServiceError(se) if se.err().is_not_found()
                );
                if is_not_found {
                    Ok(false)
                } else {
                    Err(Self::sdk_error(e))
                }
            }
        }
    }

    /// Check if path is a directory (prefix with trailing slash has children).
    async fn is_directory(&self, path: &Path) -> Result<bool, FSError> {
        let path_str = path.to_string_lossy();
        if path_str == "/" {
            return Ok(true);
        }
        let key = self.path_to_key(path);
        let prefix = format!("{}/", key);
        let list = self
            .client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .prefix(&prefix)
            .max_keys(1)
            .send()
            .await
            .map_err(Self::sdk_error)?;
        Ok(list.key_count().unwrap_or(0) > 0 || !list.common_prefixes().is_empty())
    }

    /// Create multipart upload; returns upload_id.
    async fn create_multipart_upload(&self, key: &str) -> Result<String, FSError> {
        let resp = self
            .client
            .create_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .map_err(Self::sdk_error)?;
        resp.upload_id()
            .map(String::from)
            .ok_or_else(|| FSError::Io {
                message: "CreateMultipartUpload: missing UploadId".to_string(),
            })
    }

    /// Upload one part; returns ETag (with quotes).
    async fn upload_part(
        &self,
        key: &str,
        upload_id: &str,
        part_number: i32,
        body: Vec<u8>,
    ) -> Result<String, FSError> {
        let body_stream = ByteStream::from(body);
        let resp = self
            .client
            .upload_part()
            .bucket(&self.config.bucket)
            .key(key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(body_stream)
            .send()
            .await
            .map_err(Self::sdk_error)?;
        resp.e_tag().map(String::from).ok_or_else(|| FSError::Io {
            message: "UploadPart: missing ETag".to_string(),
        })
    }

    /// Complete multipart upload.
    async fn complete_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
        etags: &[String],
    ) -> Result<(), FSError> {
        let parts: Vec<CompletedPart> = etags
            .iter()
            .enumerate()
            .map(|(i, etag)| {
                CompletedPart::builder()
                    .part_number((i + 1) as i32)
                    .e_tag(etag)
                    .build()
            })
            .collect();
        let completed = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();
        self.client
            .complete_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed)
            .send()
            .await
            .map_err(Self::sdk_error)?;
        Ok(())
    }

    /// Abort multipart upload.
    async fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> Result<(), FSError> {
        let _ = self
            .client
            .abort_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await;
        Ok(())
    }
}

#[async_trait]
impl FileSystem for S3FS {
    async fn truncate(&self, path: &Path, size: u64) -> Result<(), FSError> {
        self.validate_config()?;
        let current = self.read(path, 0, -1).await?;
        if current.len() as u64 <= size {
            return Ok(());
        }
        let truncated = current[..size as usize].to_vec();
        let key = self.path_to_key(path);
        let _ = self
            .client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(ByteStream::from(truncated))
            .send()
            .await
            .map_err(Self::sdk_error)?;
        Ok(())
    }

    async fn touch(&self, path: &Path) -> Result<(), FSError> {
        self.validate_config()?;
        let key = self.path_to_key(path);
        if key.is_empty() {
            return Ok(());
        }
        if self.exists(path).await? {
            let copy_source = format!("{}/{}", self.config.bucket, key);
            self.client
                .copy_object()
                .bucket(&self.config.bucket)
                .key(&key)
                .copy_source(copy_source)
                .send()
                .await
                .map_err(Self::sdk_error)?;
        } else {
            self.create(path).await?;
        }
        Ok(())
    }
}
