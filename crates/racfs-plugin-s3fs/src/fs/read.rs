use std::path::{Path, PathBuf};

use async_trait::async_trait;
use aws_sdk_s3::error::SdkError;
use racfs_core::{error::FSError, filesystem::ReadFS, metadata::FileMetadata};

use super::S3FS;

#[async_trait]
impl ReadFS for S3FS {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        self.validate_config()?;

        let key = self.path_to_key(path);
        if key.is_empty() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let mut get = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key);

        if offset > 0 || size > 0 {
            let range = if size > 0 {
                format!("bytes={}-{}", offset, offset + size - 1)
            } else {
                format!("bytes={}-", offset)
            };
            get = get.range(range);
        }

        let resp = get.send().await.map_err(S3FS::sdk_error)?;
        let body = resp.body;
        let bytes = body.collect().await.map_err(S3FS::sdk_error)?.into_bytes();
        Ok(bytes.to_vec())
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        self.validate_config()?;

        let path_str = path.to_string_lossy();
        if path_str == "/" {
            return Ok(FileMetadata::directory(PathBuf::from("/")));
        }

        let key = self.path_to_key(path);
        if self.is_directory(path).await? {
            return Ok(FileMetadata::directory(path.to_path_buf()));
        }

        let resp = self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await;

        match resp {
            Ok(head) => {
                let content_length = head.content_length().unwrap_or(0) as u64;
                let mut meta = FileMetadata::file(path.to_path_buf(), content_length);
                if let Some(last_modified) = head.last_modified() {
                    meta.modified = chrono::DateTime::from_timestamp(
                        last_modified.secs(),
                        last_modified.subsec_nanos(),
                    )
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                    meta.accessed = meta.modified;
                }
                Ok(meta)
            }
            Err(e) => {
                let is_not_found = matches!(
                    &e,
                    SdkError::ServiceError(se) if se.err().is_not_found()
                );
                if is_not_found {
                    if self.is_directory(path).await? {
                        return Ok(FileMetadata::directory(path.to_path_buf()));
                    }
                    Err(FSError::NotFound {
                        path: path.to_path_buf(),
                    })
                } else {
                    Err(S3FS::sdk_error(e))
                }
            }
        }
    }
}
