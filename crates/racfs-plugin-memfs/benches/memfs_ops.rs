//! MemFS operation benchmarks (create, read, write, read_dir).

use criterion::{Criterion, criterion_group, criterion_main};
use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
use racfs_core::flags::WriteFlags;
use racfs_plugin_memfs::MemFS;
use std::hint::black_box;
use std::path::Path;
use tokio::runtime::Runtime;

fn runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn bench_memfs_create(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("memfs_create_file", |b| {
        b.iter(|| {
            rt.block_on(async {
                let fs = MemFS::new();
                for i in 0..100 {
                    let s = format!("/file_{}", i);
                    fs.create(Path::new(&s)).await.unwrap();
                }
            })
        })
    });
}

fn bench_memfs_read(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("memfs_read_1k", |b| {
        b.iter(|| {
            rt.block_on(async {
                let fs = MemFS::new();
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

fn bench_memfs_read_dir(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("memfs_readdir_100", |b| {
        b.iter(|| {
            rt.block_on(async {
                let fs = MemFS::new();
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

fn bench_memfs_stat(c: &mut Criterion) {
    let rt = runtime();
    c.bench_function("memfs_stat", |b| {
        b.iter(|| {
            rt.block_on(async {
                let fs = MemFS::new();
                fs.create(Path::new("/f")).await.unwrap();
                let _ = black_box(fs.stat(Path::new("/f")).await.unwrap());
            })
        })
    });
}

criterion_group!(
    benches,
    bench_memfs_create,
    bench_memfs_read,
    bench_memfs_read_dir,
    bench_memfs_stat
);
criterion_main!(benches);
