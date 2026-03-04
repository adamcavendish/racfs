//! S3 integration example using the S3FS plugin.
//!
//! Requires a real S3 bucket or MinIO. Set environment variables:
//!   RACFS_S3_BUCKET    - bucket name (required to run against S3)
//!   AWS_ACCESS_KEY_ID  - access key (or RACFS_S3_ACCESS_KEY)
//!   AWS_SECRET_ACCESS_KEY - secret key (or RACFS_S3_SECRET_KEY)
//!   AWS_REGION         - region (default: us-east-1)
//!   RACFS_S3_ENDPOINT  - optional custom endpoint (e.g. http://localhost:9000 for MinIO)
//!
//! Run: `cargo run -p racfs-vfs --example s3_integration`
//! If credentials are not set, the example prints usage and exits successfully.

use std::path::Path;

use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
use racfs_core::flags::WriteFlags;
use racfs_plugin_s3fs::{S3Config, S3FS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bucket = std::env::var("RACFS_S3_BUCKET").ok();
    let access_key = std::env::var("RACFS_S3_ACCESS_KEY")
        .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
        .ok();
    let secret_key = std::env::var("RACFS_S3_SECRET_KEY")
        .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
        .ok();
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let endpoint = std::env::var("RACFS_S3_ENDPOINT").ok();

    if bucket.is_none() || access_key.is_none() || secret_key.is_none() {
        eprintln!("S3 integration example (skipped: no credentials)");
        eprintln!(
            "Set RACFS_S3_BUCKET, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY to run against S3 or MinIO."
        );
        eprintln!("Optional: AWS_REGION, RACFS_S3_ENDPOINT (e.g. http://localhost:9000)");
        return Ok(());
    }

    let config = S3Config {
        bucket: bucket.unwrap(),
        region: region.clone(),
        endpoint: endpoint.clone(),
        access_key: access_key.unwrap(),
        secret_key: secret_key.unwrap(),
        cache_enabled: false,
        cache_size: 0,
        multipart_threshold: 5 * 1024 * 1024,
        multipart_part_size: 5 * 1024 * 1024,
    };

    let fs = S3FS::new(config)?;
    let prefix = "/racfs-example";
    let file_path = format!("{}/hello.txt", prefix);

    println!("S3 integration: bucket configured, region={}", region);

    // Create prefix "directory" and file
    fs.mkdir(Path::new(prefix), 0o755).await?;
    fs.create(Path::new(&file_path)).await?;
    fs.write(
        Path::new(&file_path),
        b"Hello from RACFS S3 integration!",
        0,
        WriteFlags::none(),
    )
    .await?;

    println!("Wrote {}", file_path);

    // Read back
    let data = fs.read(Path::new(&file_path), 0, -1).await?;
    let s = String::from_utf8(data)?;
    println!("Read: {}", s);

    // Stat
    let meta = fs.stat(Path::new(&file_path)).await?;
    println!("Stat: size={} path={:?}", meta.size, meta.path);

    // List directory
    let entries: Vec<_> = fs.read_dir(Path::new(prefix)).await?;
    println!("List {}: {} entries", prefix, entries.len());

    // Cleanup
    fs.remove(Path::new(&file_path)).await?;
    fs.remove_all(Path::new(prefix)).await?;
    println!("Cleaned up.");

    Ok(())
}
