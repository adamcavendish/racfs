use std::path::Path;

use async_trait::async_trait;
use racfs_core::{error::FSError, filesystem::DirFS, metadata::FileMetadata};

use super::S3FS;

#[async_trait]
impl DirFS for S3FS {
    async fn mkdir(&self, path: &Path, _perm: u32) -> Result<(), FSError> {
        self.validate_config()?;

        let key = self.path_to_key(path);
        let dir_key = if key.is_empty() {
            "/".to_string()
        } else {
            format!("{}/", key)
        };

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&dir_key)
            .body(aws_sdk_s3::primitives::ByteStream::from_static(&[]))
            .send()
            .await
            .map_err(S3FS::sdk_error)?;
        Ok(())
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        self.validate_config()?;

        let path_str = path.to_string_lossy();
        if path_str == "/" {
            return self.list_objects_v2("", Some("/")).await;
        }

        let key = self.path_to_key(path);
        let prefix = if key.is_empty() {
            String::new()
        } else {
            format!("{}/", key)
        };

        self.list_objects_v2(&prefix, Some("/")).await
    }

    async fn remove(&self, path: &Path) -> Result<(), FSError> {
        self.validate_config()?;

        let key = self.path_to_key(path);
        if key.is_empty() {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(S3FS::sdk_error)?;
        Ok(())
    }

    async fn remove_all(&self, path: &Path) -> Result<(), FSError> {
        self.validate_config()?;

        let path_str = path.to_string_lossy();
        if path_str == "/" {
            return Err(FSError::PermissionDenied {
                path: path.to_path_buf(),
            });
        }

        let key = self.path_to_key(path);
        if !key.is_empty() {
            let prefix = format!("{}/", key);
            let mut continuation_token: Option<String> = None;

            loop {
                let mut list = self
                    .client
                    .list_objects_v2()
                    .bucket(&self.config.bucket)
                    .prefix(&prefix)
                    .max_keys(1000);
                if let Some(ref t) = continuation_token {
                    list = list.continuation_token(t);
                }

                let resp = list.send().await.map_err(S3FS::sdk_error)?;
                let keys: Vec<String> = resp
                    .contents()
                    .iter()
                    .filter_map(|o| o.key().map(String::from))
                    .collect();

                if !keys.is_empty() {
                    let objs: Vec<aws_sdk_s3::types::ObjectIdentifier> = keys
                        .iter()
                        .filter_map(|k| {
                            aws_sdk_s3::types::ObjectIdentifier::builder()
                                .key(k)
                                .build()
                                .ok()
                        })
                        .collect();
                    let delete_payload = aws_sdk_s3::types::Delete::builder()
                        .set_objects(Some(objs))
                        .build()
                        .map_err(S3FS::sdk_error)?;
                    let delete = self
                        .client
                        .delete_objects()
                        .bucket(&self.config.bucket)
                        .delete(delete_payload);
                    delete.send().await.map_err(S3FS::sdk_error)?;
                }

                if !resp.is_truncated().unwrap_or(false) {
                    break;
                }
                continuation_token = resp.next_continuation_token().map(String::from);
                if continuation_token.is_none() {
                    break;
                }
            }
        }

        if !key.is_empty() {
            let _ = self
                .client
                .delete_object()
                .bucket(&self.config.bucket)
                .key(&key)
                .send()
                .await;
        }

        Ok(())
    }

    async fn rename(&self, old_path: &Path, new_path: &Path) -> Result<(), FSError> {
        self.validate_config()?;

        let old_key = self.path_to_key(old_path);
        let new_key = self.path_to_key(new_path);

        if old_key.is_empty() || new_key.is_empty() {
            return Err(FSError::PermissionDenied {
                path: old_path.to_path_buf(),
            });
        }

        let copy_source = format!("{}/{}", self.config.bucket, old_key);
        self.client
            .copy_object()
            .bucket(&self.config.bucket)
            .key(&new_key)
            .copy_source(copy_source)
            .send()
            .await
            .map_err(S3FS::sdk_error)?;

        self.remove(old_path).await?;
        Ok(())
    }
}
