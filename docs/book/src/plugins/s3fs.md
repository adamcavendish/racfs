# S3FS

**S3-compatible object storage filesystem.** Uses a bucket (and optional prefix) as the backend. Supports multipart uploads for large files.

## Config

Requires S3 endpoint and credentials (e.g. bucket, region, access key). See `racfs-plugin-s3fs` and server config for `S3Config` fields (bucket, region, endpoint override, etc.).

```toml
[mounts.s3]
path = "/s3"
fs_type = "s3fs"
# ... S3-specific options (bucket, region, etc.)
```

## Behavior

- Paths map to object keys. Create/write uploads objects; read downloads them.
- Multipart uploads for large writes.
- Optional caching (e.g. Foyer) for frequently read objects; see performance-tuning docs.

## Crate

`racfs-plugin-s3fs`
