# Performance Tuning Guide

This guide covers how to measure and improve RACFS performance for your workload.

## Running Benchmarks

The MemFS plugin is benchmarked with [Criterion](https://github.com/bheisler/criterion.rs). Run:

```bash
cargo bench -p racfs-vfs
```

This runs both **memfs_ops** and **localfs_ops** (MemFS in-memory vs LocalFS on a temp directory), so you can compare backends. For a single suite:

```bash
cargo bench -p racfs-vfs --bench memfs_ops
cargo bench -p racfs-vfs --bench localfs_ops
```

Example output (approximate, machine-dependent):

| Benchmark             | Description                    | Typical order (MemFS) | Typical order (LocalFS) |
|-----------------------|--------------------------------|------------------------|--------------------------|
| `memfs_create_file`   | Create 100 files               | ~30 µs                 | —                        |
| `localfs_create_file` | Create 100 files (temp dir)   | —                      | ~12 ms                   |
| `memfs_read_1k`       | Create file, write 1 KiB, read| ~1 µs                  | —                        |
| `localfs_read_1k`     | Same for LocalFS               | —                      | ~2.5 ms                  |
| `memfs_readdir_100`   | mkdir + 100 files + readdir   | ~50 µs                 | —                        |
| `localfs_readdir_100` | Same for LocalFS               | —                      | ~12 ms                   |
| `memfs_stat`          | Create file, stat              | ~0.6 µs                | —                        |
| `localfs_stat`        | Same for LocalFS               | —                      | ~300 µs                  |

These numbers reflect **in-process** MemFS only (no network or server). Use them as a baseline; real throughput will be lower when going through the REST API and FUSE.

To run all workspace benchmarks (when more plugins are added):

```bash
cargo bench --workspace
```

CI runs `cargo bench -p racfs-vfs` on every push to catch regressions (see `.github/workflows/ci.yml`).

## FUSE Client Caching

The FUSE layer caches **metadata** (stat) and **directory listings** to reduce round-trips to the server.

- **Default TTL:** 1 second. Cached entries are reused until they expire or are invalidated.
- **Invalidation:** Create, mkdir, write, unlink, rmdir, and rename invalidate the affected path and parent directory so listings stay consistent.

Effects:

- **Higher TTL** → fewer server calls, better throughput for read-heavy workloads; stale data may show longer after changes from another client.
- **Lower TTL** → fresher view; more server load and latency.

The default `mount()` function uses a 1 s TTL. To use a custom TTL you must use the library and build the FUSE filesystem yourself with a custom cache:

```rust
use racfs_fuse::{RacfsAsyncFs, FuseCache, TokioAdapter};
use std::time::Duration;

// Longer TTL for read-mostly workloads
let cache = FuseCache::new(Duration::from_secs(5));
let fs = RacfsAsyncFs::with_cache("http://127.0.0.1:8080", cache)?;
// Then use fuser's TokioAdapter with this fs and mount2...
```

See [FUSE usage](fuse-usage.md#caching) for a short example.

## Server-side read cache (CachedFs and FoyerCache)

For backends where **read**, **stat**, or **readdir** are expensive (e.g. network or disk), you can wrap any `FileSystem` with **CachedFs** and a **Cache** implementation so that repeated reads and listings are served from memory.

- **HashMapCache** (racfs-core): Unbounded in-memory cache; good for tests or when the working set is small.
- **FoyerCache** (racfs-core): Bounded in-memory cache backed by [foyer](https://docs.rs/foyer) with configurable eviction (e.g. LRU, LFU, S3Fifo). Use for production when you need bounded capacity. For hybrid (memory + disk) use foyer's `HybridCache` in a custom wrapper. Example: `cargo run -p racfs-core --example foyer_cache_usage`.

Example (see `crates/racfs-vfs/examples/cached_fs_usage.rs`):

```rust
use racfs_core::{Cache, HashMapCache};
use racfs_vfs::CachedFs;
use racfs_plugin_memfs::MemFS;
use std::sync::Arc;

let inner = Arc::new(MemFS::new());
let cache = Arc::new(HashMapCache::new());
let cached = CachedFs::new(inner, cache);
// Use `cached` as a FileSystem; read/stat/read_dir are cached.
// For bounded cache use FoyerCache::new(1000).
```

Use **`with_key_prefix("mount_path:")`** when multiple CachedFs instances share the same cache (e.g. per mount) so keys do not collide. Mutations (create, write, remove, rename, etc.) invalidate the relevant cache entries automatically.

### Foyer hybrid (memory + disk)

For **memory + disk** tiers (e.g. hot data in RAM, cold data on disk), [foyer](https://docs.rs/foyer) provides `HybridCache`. RACFS does not ship a built-in `HybridCache` wrapper; you can implement the [`Cache`](https://docs.rs/racfs-core/latest/racfs_core/trait.Cache.html) trait in your crate by wrapping `foyer::HybridCache` the same way racfs-core wraps `foyer::Cache` for `FoyerCache`: implement `get` (return `Vec<u8>`), `put`, and `remove`; `invalidate_prefix` can be a no-op or implemented by tracking keys if your use case needs prefix invalidation. Then pass your wrapper to **CachedFs** or use it in a custom plugin. See `crates/racfs-core/src/cache.rs` (FoyerCache) for the in-memory pattern and [foyer’s docs](https://docs.rs/foyer) for HybridCache configuration (memory capacity, disk path, eviction policies).

**S3FS cache tiers:** For S3-backed mounts, enable a cache tier in server config: set `[mounts.<name>.cache]` with `enabled = true` and `max_entries = <n>`. The server wraps the backend with **CachedFs** and **FoyerCache**.

**S3FS prefetch and write-through:** Planned: read-ahead after a read; update cache with written range after a write (currently CachedFs invalidates on write).

## Backend Choice

| Backend   | Latency        | Use case                          |
|-----------|----------------|------------------------------------|
| **MemFS** | Lowest (in-process) | Dev, tests, ephemeral data         |
| **LocalFS** | Local disk I/O | Single-node, file-based storage   |
| **S3FS**  | Network + S3   | Scalable, durable object storage  |
| **HttpFS**| Network        | Read-only HTTP resources          |
| **StreamFS** | In-memory streams | Real-time / streaming data      |

For best throughput on a single machine, use MemFS or LocalFS and avoid network. For shared or durable storage, use S3FS or similar; then FUSE cache TTL and (in the future) server-side caching (e.g. Foyer) will matter more.

## Transparent compression (zstd)

RACFS supports transparent compression via the [`Compression`](https://docs.rs/racfs-core/latest/racfs_core/trait.Compression.html) trait and **ZstdCompression** in racfs-core. Compression is always compiled in (one binary); enable or disable it at **runtime via config**. For example, in StreamFS set `compression: None` to disable or `compression: Some(Arc::new(ZstdCompression::new(CompressionLevel::Default)))` to enable. No Cargo features are required—ship a single binary and turn compression on or off in configuration.

## Server and Build

- **Release build:** Always use `cargo build --release` and run the server and FUSE binary in release mode for production.
- **Concurrency:** The server uses async I/O; one process can serve many concurrent requests. Tune the number of worker threads or connections if you profile bottlenecks.
- **StreamFS:** If you use StreamFS, `buffer_size` and `history_size` in server config affect memory and how much history is kept; larger values can improve consumers but increase memory use.

## HTTP Client Connection Pooling

The `racfs-client` uses [reqwest](https://docs.rs/reqwest), which enables **connection pooling by default** (typically 10 idle connections per host, 90 s idle timeout). For high concurrency (e.g. many FUSE ops or a load-test client), you can increase the pool size with `Client::builder()`:

```rust
use racfs_client::Client;
use std::time::Duration;

// Default client (pool size 10 per host)
let client = Client::new("http://127.0.0.1:8080");

// Tuned for many concurrent requests
let client = Client::builder()
    .pool_max_idle_per_host(32)
    .pool_idle_timeout(Duration::from_secs(90))
    .build("http://127.0.0.1:8080");
```

Use a larger `pool_max_idle_per_host` (e.g. 32 or 64) when you run many concurrent API calls; otherwise the default is sufficient.

## Async I/O tuning

Hot paths (FUSE read/write, server request handlers, client HTTP calls) should stay async and avoid blocking the runtime:

- **Avoid blocking in async:** Do not use `std::thread::sleep`, synchronous file I/O, or long CPU-bound work inside async functions without offloading. Use `tokio::time::sleep`, `tokio::fs`, and `tokio::task::spawn_blocking` for blocking work.
- **Profile:** Build with `cargo build --release` and use [tracing](https://docs.rs/tracing) or a flamegraph (e.g. [cargo-flamegraph](https://github.com/flamegraph-rs/flamegraph)) to find blocking or high-latency sections. The server’s latency middleware records per-request duration by operation.
- **Concurrency:** The server and FUSE client use Tokio; ensure the runtime has enough worker threads for your workload (default is often sufficient; increase via `tokio::runtime::Builder::worker_threads` if needed).

See the [Zero-copy and allocation](#zero-copy-and-allocation) section for related optimization notes.

## Zero-copy and allocation

Planned and partial improvements (see [ROADMAP](../ROADMAP.md) Performance Optimization):

- **Zero-copy:** Prefer `Bytes` or `bytes::Bytes` in hot paths instead of `Vec<u8>` where buffers are passed through without modification, to avoid unnecessary copies and allocations. The REST API and client already use bytes where appropriate; FUSE read/write buffers can be reviewed for similar gains.
- **Async I/O:** Ensure hot paths (FUSE read/write, client request pipeline) use async I/O and avoid blocking; profile with `cargo build --release` and a tracing or flamegraph tool to find blocking sections.
- **Allocation reduction:** Reuse buffers where possible, preallocate when size is known, and avoid unnecessary clones in request/response handling. Use `cargo build --release` with a memory profiler to identify allocation hotspots.

## FUSE 0.1.0 (advanced features)

Planned and implemented (see [ROADMAP](../ROADMAP.md) FUSE Foundation):

- **File locking:** Implemented. `getlk` / `setlk` / `setlkw` support advisory POSIX locks (flock, lockf) process-locally; lock state is tracked per inode in the FUSE layer.
- **mmap:** Supported. getattr, read, and write are coherent with the kernel page cache so that memory-mapped reads work; for write-back of mapped writes use `msync()`. No FUSE3-specific mmap ops required for basic use.
- **FUSE3 / libfuse3:** The fuser crate is already compatible with FUSE3 (libfuse3). Enable the `libfuse3` feature when building racfs-fuse to link against libfuse3 for mount/umount; otherwise on Linux, fuser uses a pure-Rust mount.
- **Multi-mount:** Allow a single process to serve multiple mount points with shared or per-mount state.
- **Zero-copy / allocation:** Apply the same zero-copy and allocation guidelines above to the FUSE layer (e.g. use `Bytes` in read/write paths, avoid extra copies in the client and inode manager).

## Test coverage

The project aims for **90%+ test coverage** (see [ROADMAP.md](../ROADMAP.md) v1.0.0 Testing Excellence). To measure coverage locally:

1. Install [cargo-tarpaulin](https://github.com/codecov/cargo-tarpaulin):  
   `cargo install cargo-tarpaulin`
2. Run the coverage script from the repo root:  
   `just coverage`  
   For an HTML report in `coverage/`:  
   `just coverage-html`
3. Or run tarpaulin directly:  
   `cargo tarpaulin --workspace --out Stdout`  
   For HTML:  
   `cargo tarpaulin --workspace --out Html --output-dir coverage`

Tarpaulin runs the test suite with instrumentation; the report shows line coverage per crate. Add unit and integration tests for untested paths to move toward the 90% target. Property-based tests (proptest) and the load-test example also contribute: run `cargo test --workspace` and see `crates/racfs-client/examples/load_test.rs`.

## Future Improvements (Roadmap)

Planned in v0.3.0 and later:

- **Foyer cache:** Hybrid in-memory and disk cache for plugins (e.g. S3FS) to improve repeated reads.
- **Read-ahead:** Sequential read buffering on the FUSE client.
- **More benchmarks:** Comparison across plugins (MemFS vs LocalFS) is in place; S3FS benchmarks are optional—set `RACFS_S3_BENCH_ENDPOINT` and `RACFS_S3_BENCH_BUCKET` (e.g. LocalStack) and run `cargo bench -p racfs-vfs --bench s3fs_ops`.

Planned in v1.0.0 (Performance Optimization):

- **Async I/O optimization:** Deepen use of async in hot paths (e.g. FUSE read/write, client request pipeline) to reduce blocking and improve throughput under concurrency.
- **Zero-copy operations:** Where possible, avoid copying buffers (e.g. `Bytes` instead of `Vec<u8>`, or splice/sendfile-style paths) to cut CPU and memory use.
- **Memory allocation optimization:** Reduce allocations in hot paths (reuse buffers, preallocate when size is known, avoid unnecessary clones in request/response handling); profile with `cargo build --release` and a memory profiler to find candidates.

See [ROADMAP.md](../ROADMAP.md) for the full list and status.

## Quick Checklist

- [ ] Run `cargo bench -p racfs-vfs` to establish a baseline after changes.
- [ ] Use release builds for server and FUSE in production.
- [ ] Prefer MemFS or LocalFS when you do not need remote/durable storage.
- [ ] Increase FUSE cache TTL (via custom `FuseCache`) if the workload is read-heavy and eventual consistency is acceptable.
- [ ] Monitor server and FUSE process CPU/memory under load.
- [x] Run property-based tests with `cargo test --workspace` (racfs-core flags, racfs-client request roundtrips).
- [ ] Check test coverage with `cargo tarpaulin --workspace --out Stdout` when adding or changing code; aim for 90%+ (see [ROADMAP](../ROADMAP.md)).
