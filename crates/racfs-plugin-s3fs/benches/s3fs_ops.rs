//! S3FS operation benchmarks (optional: require S3 endpoint and bucket).
//!
//! Set environment variables to run against a real S3-compatible endpoint (e.g. LocalStack):
//!
//! - `RACFS_S3_BENCH_ENDPOINT` – endpoint URL (e.g. `http://localhost:4566`)
//! - `RACFS_S3_BENCH_BUCKET` – bucket name (create it first, e.g. `aws s3 mb s3://bench-bucket --endpoint-url http://localhost:4566`)
//! - `RACFS_S3_BENCH_ACCESS_KEY` – optional (default empty for LocalStack)
//! - `RACFS_S3_BENCH_SECRET_KEY` – optional (default empty for LocalStack)
//!
//! Without these set, benchmarks no-op so `cargo bench -p racfs-plugin-s3fs` still passes.

use criterion::{Criterion, criterion_group, criterion_main};
use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
use racfs_plugin_s3fs::{S3Config, S3FS};
use std::hint::black_box;
use std::path::Path;
use tokio::runtime::Runtime;

fn runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn s3_config_from_env() -> Option<S3Config> {
    let endpoint = std::env::var("RACFS_S3_BENCH_ENDPOINT").ok()?;
    let bucket = std::env::var("RACFS_S3_BENCH_BUCKET").ok()?;
    if endpoint.is_empty() || bucket.is_empty() {
        return None;
    }
    let access_key = std::env::var("RACFS_S3_BENCH_ACCESS_KEY").unwrap_or_default();
    let secret_key = std::env::var("RACFS_S3_BENCH_SECRET_KEY").unwrap_or_default();
    Some(S3Config {
        bucket,
        region: "us-east-1".to_string(),
        endpoint: Some(endpoint),
        access_key,
        secret_key,
        cache_enabled: false,
        cache_size: 0,
        multipart_threshold: 5 * 1024 * 1024,
        multipart_part_size: 5 * 1024 * 1024,
    })
}

fn bench_s3fs_stat(c: &mut Criterion) {
    let rt = runtime();
    let config = s3_config_from_env();
    c.bench_function("s3fs_stat", |b| {
        b.iter(|| {
            rt.block_on(async {
                if let Some(cfg) = config.as_ref() {
                    let fs = S3FS::new(cfg.clone()).expect("bench: S3FS::new");
                    let _ = black_box(fs.stat(Path::new("/")).await);
                } else {
                    black_box(());
                }
            })
        })
    });
}

fn bench_s3fs_create_and_stat(c: &mut Criterion) {
    let rt = runtime();
    let config = s3_config_from_env();
    c.bench_function("s3fs_create_and_stat", |b| {
        b.iter(|| {
            rt.block_on(async {
                if let Some(cfg) = config.as_ref() {
                    let fs = S3FS::new(cfg.clone()).expect("bench: S3FS::new");
                    let path = format!(
                        "/bench_{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_nanos()
                    );
                    let _ = fs.create(Path::new(&path)).await;
                    let _ = black_box(fs.stat(Path::new(&path)).await);
                } else {
                    black_box(());
                }
            })
        })
    });
}

fn bench_s3fs_read_dir(c: &mut Criterion) {
    let rt = runtime();
    let config = s3_config_from_env();
    c.bench_function("s3fs_readdir", |b| {
        b.iter(|| {
            rt.block_on(async {
                if let Some(cfg) = config.as_ref() {
                    let fs = S3FS::new(cfg.clone()).expect("bench: S3FS::new");
                    let _ = black_box(fs.read_dir(Path::new("/")).await);
                } else {
                    black_box(());
                }
            })
        })
    });
}

criterion_group!(
    benches,
    bench_s3fs_stat,
    bench_s3fs_create_and_stat,
    bench_s3fs_read_dir
);
criterion_main!(benches);
