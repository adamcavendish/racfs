use std::path::Path;

use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use racfs_core::{
    error::FSError,
    filesystem::{ReadFS, WriteFS},
    flags::WriteFlags,
};

use super::S3FS;

#[async_trait]
impl WriteFS for S3FS {
    async fn create(&self, path: &Path) -> Result<(), FSError> {
        self.validate_config()?;

        let key = self.path_to_key(path);
        if key.is_empty() {
            return Ok(());
        }

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(ByteStream::from_static(&[]))
            .send()
            .await
            .map_err(S3FS::sdk_error)?;
        Ok(())
    }

    async fn write(
        &self,
        path: &Path,
        data: &[u8],
        offset: i64,
        flags: WriteFlags,
    ) -> Result<u64, FSError> {
        self.validate_config()?;

        let key = self.path_to_key(path);
        if key.is_empty() {
            return Err(FSError::IsDirectory {
                path: path.to_path_buf(),
            });
        }

        let mut final_data = data.to_vec();

        if offset > 0 || flags.contains_append() {
            let existing = self.read(path, 0, -1).await.unwrap_or_default();
            if offset > 0 && flags.contains_append() {
                final_data = existing;
                final_data.extend_from_slice(data);
            } else if offset > 0 {
                let mut new_data = existing;
                if (offset as usize) < new_data.len() {
                    new_data.truncate(offset as usize);
                }
                new_data.extend_from_slice(data);
                final_data = new_data;
            }
        }

        let size = final_data.len();
        let threshold = self.config.multipart_threshold;
        let part_size = self.config.multipart_part_size;

        if size >= threshold && part_size > 0 {
            let upload_id = self.create_multipart_upload(&key).await?;
            let mut etags = Vec::new();
            let mut part_number: i32 = 1;
            let mut offset_pos = 0;

            while offset_pos < size {
                let end = (offset_pos + part_size).min(size);
                let part_data = final_data[offset_pos..end].to_vec();
                match self
                    .upload_part(&key, &upload_id, part_number, part_data)
                    .await
                {
                    Ok(etag) => {
                        etags.push(etag);
                        part_number += 1;
                        offset_pos = end;
                    }
                    Err(e) => {
                        let _ = self.abort_multipart_upload(&key, &upload_id).await;
                        return Err(e);
                    }
                }
            }

            if let Err(e) = self
                .complete_multipart_upload(&key, &upload_id, &etags)
                .await
            {
                let _ = self.abort_multipart_upload(&key, &upload_id).await;
                return Err(e);
            }
        } else {
            self.client
                .put_object()
                .bucket(&self.config.bucket)
                .key(&key)
                .body(ByteStream::from(final_data))
                .send()
                .await
                .map_err(S3FS::sdk_error)?;
        }

        Ok(data.len() as u64)
    }
}
