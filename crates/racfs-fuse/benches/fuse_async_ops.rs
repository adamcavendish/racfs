//! Benchmarks for FUSE async operations (RacfsAsyncFs) against an in-process server.
//!
//! Compares sequential (block_on per call) vs concurrent (join_all) to show the benefit
//! of the async path when the FUSE layer uses TokioAdapter (spawns per request).

use criterion::{Criterion, criterion_group, criterion_main};
use racfs_fuse::{AsyncFilesystemCompat, RacfsAsyncFs};
use racfs_server::api::{directories, files, health, metadata};
use racfs_server::auth::JwtConfig;
use racfs_server::state::AppState;
use racfs_plugin_devfs::DevFS;
use racfs_plugin_memfs::MemFS;
use racfs_vfs::MountableFS;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::{delete, get, post, put};

async fn start_bench_server() -> (String, tokio::task::JoinHandle<()>) {
    let vfs = Arc::new(MountableFS::new());
    let memfs = Arc::new(MemFS::new()) as Arc<dyn racfs_core::FileSystem>;
    let devfs = Arc::new(DevFS::new()) as Arc<dyn racfs_core::FileSystem>;
    vfs.mount(PathBuf::from("/memfs"), memfs).unwrap();
    vfs.mount(PathBuf::from("/dev"), devfs).unwrap();

    let state = AppState::new(vfs, JwtConfig::default(), 2, vec![]).expect("AppState::new");
    let app = Router::new()
        .route("/api/v1/health", get(health::health_check))
        .route("/api/v1/files", get(files::read_file))
        .route("/api/v1/files", post(files::create_file))
        .route("/api/v1/files", put(files::write_file))
        .route("/api/v1/files", delete(files::delete_file))
        .route("/api/v1/directories", get(directories::list_directory))
        .route("/api/v1/directories", post(directories::create_directory))
        .route("/api/v1/stat", get(metadata::stat))
        .route("/api/v1/rename", post(metadata::rename))
        .route("/api/v1/chmod", post(metadata::chmod))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (base_url, handle)
}

fn bench_lookup_sequential(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (base_url, _handle) = rt.block_on(start_bench_server());
    let fs = Arc::new(RacfsAsyncFs::new(&base_url).unwrap());

    c.bench_function("fuse_async_lookup_sequential_100", |b| {
        b.iter(|| {
            rt.block_on(async {
                for _ in 0..100 {
                    let _ = black_box(fs.lookup_async(1, std::ffi::OsStr::new("memfs")).await);
                }
            })
        })
    });
}

fn bench_lookup_concurrent(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (base_url, _handle) = rt.block_on(start_bench_server());
    let fs = Arc::new(RacfsAsyncFs::new(&base_url).unwrap());

    c.bench_function("fuse_async_lookup_concurrent_100", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut tasks = Vec::with_capacity(100);
                for _ in 0..100 {
                    let fs = fs.clone();
                    tasks.push(tokio::spawn(async move {
                        fs.lookup_async(1, std::ffi::OsStr::new("memfs")).await
                    }));
                }
                for t in tasks {
                    let _ = black_box(t.await.unwrap());
                }
            })
        })
    });
}

fn bench_getattr_sequential(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (base_url, _handle) = rt.block_on(start_bench_server());
    let fs = Arc::new(RacfsAsyncFs::new(&base_url).unwrap());
    let _ = rt.block_on(fs.lookup_async(1, std::ffi::OsStr::new("memfs")));

    c.bench_function("fuse_async_getattr_sequential_100", |b| {
        b.iter(|| {
            rt.block_on(async {
                for _ in 0..100 {
                    let _ = black_box(fs.getattr_async(1).await);
                }
            })
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets =
        bench_lookup_sequential,
        bench_lookup_concurrent,
        bench_getattr_sequential,
}
criterion_main!(benches);
