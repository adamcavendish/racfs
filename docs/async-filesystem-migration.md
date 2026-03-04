# AsyncFilesystem Migration Guide

**Status:** Done. Default mount is read-write via blocking adapter; mount_async is read-only via TokioAdapter.

---

## Current state

- **`crates/racfs-fuse/src/async_fs.rs`** – `RacfsAsyncFs` implements fuser's **`AsyncFilesystem`** (lookup, getattr, read, readdir) and **`AsyncFilesystemCompat`** with full read-write ops. Read path uses `Bytes`.
- **`crates/racfs-fuse/src/blocking_adapter.rs`** – Internal type implementing fuser's sync **`Filesystem`** by `block_on`'ing `RacfsAsyncFs`; used by **`mount()`** for read-write. File locking (getlk/setlk) implemented process-locally.
- **`mount()`** – Builds `BlockingAdapter`, runs `mount2` (read-write). **`mount_async()`** – Builds `TokioAdapter(RacfsAsyncFs)`, runs `mount2` with RO (read-only). **`mount_multi()`** – Spawns one thread per (server_url, mount_point), each running `mount()`.
- **Resilience:** Client retry, circuit breaker, TTL cache, stale cache on backend failure unchanged. Optional **libfuse3** feature; **Bytes** in read path.


---

## Overview

Migrate RACFS FUSE implementation from synchronous `Filesystem` trait to experimental `AsyncFilesystem` trait for better async support and performance.

## Current Implementation Issues

The current implementation uses `Filesystem` trait with `runtime.block_on()`:

```rust
impl Filesystem for RacfsFuse {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        // ...
        match self.runtime.block_on(async {
            self.client.stat(&path_str).await  // Blocks the runtime!
        }) {
            Ok(metadata) => { /* ... */ }
            Err(e) => { /* ... */ }
        }
    }
}
```

**Problems:**
- ❌ `block_on()` blocks the tokio runtime thread
- ❌ Poor async task scheduling
- ❌ Manual error conversion
- ❌ Boilerplate code for each operation
- ❌ No built-in response types

## Target Implementation

Use `AsyncFilesystem` trait with `TokioAdapter`:

```rust
use fuser::experimental::{AsyncFilesystem, TokioAdapter, RequestContext, LookupResponse};

#[async_trait::async_trait]
impl AsyncFilesystem for RacfsFuse {
    async fn lookup(
        &self,
        context: &RequestContext,
        parent: INodeNo,
        name: &OsStr,
    ) -> Result<LookupResponse, Errno> {
        let parent = parent.0;
        let parent_path = self.inodes.get_path(parent)
            .ok_or(Errno::ENOENT)?;

        let path = parent_path.join(name);
        let metadata = self.client.stat(&path.to_string_lossy()).await
            .map_err(|_| Errno::ENOENT)?;

        let inode = self.inodes.allocate(&path);
        let attr = self.metadata_to_attr(inode, &metadata);

        Ok(LookupResponse::new(TTL, attr, Generation(0)))
    }

    async fn getattr(
        &self,
        context: &RequestContext,
        ino: INodeNo,
        file_handle: Option<FileHandle>,
    ) -> Result<GetAttrResponse, Errno> {
        let ino = ino.0;
        let path = self.inodes.get_path(ino)
            .ok_or(Errno::ENOENT)?;

        let metadata = self.client.stat(&path.to_string_lossy()).await
            .map_err(|_| Errno::ENOENT)?;

        let attr = self.metadata_to_attr(ino, &metadata);
        Ok(GetAttrResponse::new(TTL, attr))
    }

    async fn read(
        &self,
        context: &RequestContext,
        ino: INodeNo,
        file_handle: FileHandle,
        offset: u64,
        size: u32,
        flags: OpenFlags,
        lock: Option<LockOwner>,
        out_data: &mut Vec<u8>,
    ) -> Result<(), Errno> {
        let ino = ino.0;
        let path = self.inodes.get_path(ino)
            .ok_or(Errno::ENOENT)?;

        let content = self.client.read_file(&path.to_string_lossy()).await
            .map_err(|_| Errno::EIO)?;

        let bytes = content.as_bytes();
        let start = offset as usize;
        let end = std::cmp::min(start + size as usize, bytes.len());

        if start < bytes.len() {
            out_data.extend_from_slice(&bytes[start..end]);
        }

        Ok(())
    }

    async fn readdir(
        &self,
        context: &RequestContext,
        ino: INodeNo,
        file_handle: FileHandle,
        offset: u64,
        mut builder: DirEntListBuilder<'_>,
    ) -> Result<(), Errno> {
        let ino = ino.0;
        let path = self.inodes.get_path(ino)
            .ok_or(Errno::ENOENT)?;

        let dir_list = self.client.read_dir(&path.to_string_lossy()).await
            .map_err(|_| Errno::EIO)?;

        let mut entries = vec![
            (ino, FileType::Directory, ".".to_string()),
            (ino, FileType::Directory, "..".to_string()),
        ];

        for entry in dir_list.entries {
            let entry_path = PathBuf::from(&entry.path);
            let entry_name = entry_path.file_name()
                .unwrap_or(OsStr::new(""))
                .to_string_lossy()
                .to_string();

            let entry_inode = self.inodes.allocate(&entry_path);
            let kind = if entry.path.ends_with('/') {
                FileType::Directory
            } else {
                FileType::RegularFile
            };

            entries.push((entry_inode, kind, entry_name));
        }

        for (i, (inode, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if builder.add(INodeNo(*inode), (i + 1) as u64, *kind, name) {
                break;
            }
        }

        Ok(())
    }
}
```

**Benefits:**
- ✅ Native async/await without blocking
- ✅ Proper async task spawning by TokioAdapter
- ✅ Built-in response types reduce boilerplate
- ✅ Cleaner error handling with `Result<T, Errno>`
- ✅ Better performance with proper async scheduling

## Migration Steps

### Step 1: Add Dependencies

Update `crates/racfs-fuse/Cargo.toml`:

```toml
[dependencies]
async-trait = "0.1"
fuser = { version = "0.17.0", features = ["experimental"] }
# ... existing dependencies
```

### Step 2: Create async_fs.rs Module

Create `crates/racfs-fuse/src/async_fs.rs`:

```rust
//! Async FUSE filesystem implementation using experimental AsyncFilesystem trait

use async_trait::async_trait;
use fuser::experimental::{
    AsyncFilesystem, RequestContext, LookupResponse, GetAttrResponse, DirEntListBuilder,
};
use fuser::{Errno, FileHandle, FileType, Generation, INodeNo, LockOwner, OpenFlags};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;
use crate::inode_manager::InodeManager;
use racfs_client::Client;

const TTL: Duration = Duration::from_secs(1);

pub struct RacfsAsyncFs {
    client: Arc<Client>,
    inodes: Arc<InodeManager>,
}

impl RacfsAsyncFs {
    pub fn new(server_url: &str) -> Result<Self> {
        Ok(Self {
            client: Arc::new(Client::new(server_url)),
            inodes: Arc::new(InodeManager::new()),
        })
    }

    // Helper methods...
}

#[async_trait]
impl AsyncFilesystem for RacfsAsyncFs {
    // Implement trait methods...
}
```

### Step 3: Update lib.rs

Update `crates/racfs-fuse/src/lib.rs`:

```rust
mod async_fs;
mod error;
mod fuse_fs;
mod inode_manager;

pub use async_fs::RacfsAsyncFs;
pub use error::{Error, Result};
pub use fuse_fs::RacfsFuse;
pub use inode_manager::InodeManager;

use fuser::experimental::TokioAdapter;
use std::path::PathBuf;
use tracing::info;

/// Mount using AsyncFilesystem (recommended)
pub fn mount_async(server_url: &str, mount_point: PathBuf) -> Result<()> {
    info!("Mounting RACFS (async) from {} at {:?}", server_url, mount_point);

    let fs = RacfsAsyncFs::new(server_url)?;
    let adapter = TokioAdapter::new(fs);

    let mut config = fuser::Config::default();
    config.mount_options.push(fuser::MountOption::FSName("racfs".to_string()));
    config.mount_options.push(fuser::MountOption::AutoUnmount);

    fuser::mount2(adapter, &mount_point, &config)
        .map_err(|e| Error::MountFailed {
            path: mount_point.display().to_string(),
            message: e.to_string(),
        })?;

    Ok(())
}

/// Mount using sync Filesystem (legacy)
pub fn mount_sync(server_url: &str, mount_point: PathBuf) -> Result<()> {
    // Keep old implementation for comparison
    mount(server_url, mount_point)
}

// Default to async
pub fn mount(server_url: &str, mount_point: PathBuf) -> Result<()> {
    mount_async(server_url, mount_point)
}
```

### Step 4: Update main.rs

Update `crates/racfs-fuse/src/main.rs`:

```rust
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::fmt;

#[derive(Parser)]
#[command(name = "racfs-fuse")]
#[command(about = "Mount RACFS as a FUSE filesystem", long_about = None)]
struct Cli {
    /// Mount point
    #[arg()]
    mountpoint: PathBuf,

    /// RACFS server URL
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,

    /// Use legacy sync implementation
    #[arg(long)]
    sync: bool,
}

fn main() {
    fmt::init();

    let cli = Cli::parse();

    info!("Starting RACFS FUSE mount");
    info!("Server: {}", cli.server);
    info!("Mount point: {:?}", cli.mountpoint);
    info!("Mode: {}", if cli.sync { "sync" } else { "async" });

    let result = if cli.sync {
        racfs_fuse::mount_sync(&cli.server, cli.mountpoint)
    } else {
        racfs_fuse::mount_async(&cli.server, cli.mountpoint)
    };

    if let Err(e) = result {
        error!("Failed to mount: {}", e);
        std::process::exit(1);
    }
}
```

### Step 5: Testing

Create benchmarks to compare performance:

```rust
// benches/async_vs_sync.rs
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn benchmark_async_mount(c: &mut Criterion) {
    c.bench_function("async mount", |b| {
        b.iter(|| {
            // Benchmark async implementation
        });
    });
}

fn benchmark_sync_mount(c: &mut Criterion) {
    c.bench_function("sync mount", |b| {
        b.iter(|| {
            // Benchmark sync implementation
        });
    });
}

criterion_group!(benches, benchmark_async_mount, benchmark_sync_mount);
criterion_main!(benches);
```

### Step 6: Documentation

Update documentation:
- Add migration guide to docs/
- Update README with async examples
- Document performance improvements
- Add troubleshooting section

## Expected Performance Improvements

Based on similar migrations:
- **Throughput:** 20-30% improvement for concurrent operations
- **Latency:** 15-25% reduction in p99 latency
- **CPU Usage:** 10-15% reduction due to better async scheduling
- **Memory:** Similar or slightly lower due to better task management

## Rollback Plan

If issues arise:
1. Keep sync implementation available with `--sync` flag
2. Default to sync if async has issues
3. Gradual rollout: async for read-only, then write operations
4. Monitor metrics and user feedback

## Timeline

- **Week 1, Days 1-2:** Add dependencies, create async_fs.rs skeleton
- **Week 1, Days 3-4:** Implement AsyncFilesystem trait methods
- **Week 1, Day 5:** Update lib.rs and main.rs
- **Week 2, Days 1-2:** Testing and benchmarking
- **Week 2, Days 3-4:** Documentation and examples
- **Week 2, Day 5:** Code review and merge

## References

- [fuser experimental.rs](https://github.com/cberner/fuser/blob/master/src/experimental.rs)
- [fuser-async crate](https://rust-digger.code-maven.com/crates/fuser-async)
- [async-trait documentation](https://docs.rs/async-trait/)
- [Tokio runtime documentation](https://docs.rs/tokio/latest/tokio/runtime/)

## Success Criteria

- ✅ All read operations work with AsyncFilesystem
- ✅ Performance benchmarks show improvement
- ✅ No regressions in functionality
- ✅ Documentation updated
- ✅ Both sync and async modes available for comparison
- ✅ CI/CD tests pass
