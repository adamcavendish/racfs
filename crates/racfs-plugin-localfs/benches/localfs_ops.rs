//! LocalFS operation benchmarks (create, read, write, read_dir).
//!
//! Uses a temporary directory per iteration so results are comparable to memfs_ops.
//! Run: cargo bench -p racfs-plugin-localfs --bench localfs_ops

use criterion::{Criterion, criterion_group, criterion_main};
use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
use racfs_core::flags::WriteFlags;
use racfs_plugin_localfs::LocalFS;
use std::hint::black_box;
use std::path::Path;
use tokio::runtime::Runtime;

fn runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn bench_localfs_create(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("localfs_create_file", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dir = tempfile::tempdir().unwrap();
                let fs = LocalFS::new(dir.path().to_path_buf());
                for i in 0..100 {
                    let s = format!("/file_{}", i);
                    fs.create(Path::new(&s)).await.unwrap();
                }
            })
        })
    });
}

fn bench_localfs_read(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("localfs_read_1k", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dir = tempfile::tempdir().unwrap();
                let fs = LocalFS::new(dir.path().to_path_buf());
                fs.create(Path::new("/f")).await.unwrap();
                let data = vec![0u8; 1024];
                fs.write(Path::new("/f"), &data, 0, WriteFlags::none())
                    .await
                    .unwrap();
                let _ = black_box(fs.read(Path::new("/f"), 0, -1).await.unwrap());
            })
        })
    });
}

fn bench_localfs_read_dir(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("localfs_readdir_100", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dir = tempfile::tempdir().unwrap();
                let fs = LocalFS::new(dir.path().to_path_buf());
                fs.mkdir(Path::new("/dir"), 0o755).await.unwrap();
                for i in 0..100 {
                    let s = format!("/dir/f{}", i);
                    fs.create(Path::new(&s)).await.unwrap();
                }
                let _ = black_box(fs.read_dir(Path::new("/dir")).await.unwrap());
            })
        })
    });
}

fn bench_localfs_stat(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("localfs_stat", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dir = tempfile::tempdir().unwrap();
                let fs = LocalFS::new(dir.path().to_path_buf());
                fs.create(Path::new("/f")).await.unwrap();
                let _ = black_box(fs.stat(Path::new("/f")).await.unwrap());
            })
        })
    });
}

criterion_group!(
    benches,
    bench_localfs_create,
    bench_localfs_read,
    bench_localfs_read_dir,
    bench_localfs_stat
);
criterion_main!(benches);
