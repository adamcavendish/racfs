# RACFS Public API Review

This document reviews the public API surface of each crate as of the v1.0.0 API stabilization effort. It serves as the baseline for semantic versioning and deprecation decisions.

## Scope

- **Stable**: Intended to remain backward-compatible; breaking changes require a major version bump.
- **Evolving**: May see additive or minor breaking changes before 1.0.
- **Internal**: Primarily for use within the workspace; external use is possible but not guaranteed.

---

## racfs-core

**Role:** Foundational types and traits for all RACFS backends and clients.

| Item | Kind | Stability | Notes |
|------|------|-----------|--------|
| `FileSystem` | trait | Stable | Core async trait; all plugins implement this. Method set is the primary contract. |
| `FSError` | enum | Stable | Error type for filesystem operations; variants are part of the contract. |
| Method return type | — | Stable | All `FileSystem` methods return `Result<T, FSError>`. |
| `FileMetadata` | struct | Stable | Path, size, mode, timestamps; serialization used by server/client. |
| `OpenFlags` | flags | Stable | Bitflags for open behavior. |
| `WriteFlags` | flags | Stable | Bitflags for write (e.g. append). |
| `Cache` | trait | Evolving | Cache abstraction; may gain async or new methods when Foyer is integrated. |
| `CacheStats` | struct | Stable | hits, misses, evictions; used by metrics. |
| `HashMapCache` | struct | Stable | Reference in-memory implementation of `Cache`. |
| `FileHandle`, `HandleId` | types | Stable | Used by `HandleFS` and handle-based APIs. |
| `Compression`, `CompressionLevel` | trait/enum | Stable | Zstd compression; enable/disable at runtime via config. |
| `ZstdCompression` | type | Stable | Built-in compressor; always compiled in. |
| `HandleFS`, `Symlinker`, `Toucher`, `XattrFS`, `Linker`, `RealPathFS` | traits | Stable | Optional extension traits; implement only if backend supports them. |

**Recommendation:** Treat `FileSystem`, `FSError`, `Result<T, FSError>`, `FileMetadata`, and flags as stable. Mark any future breaking changes to `FileSystem` method signatures with deprecation cycles.

---

## racfs-client

**Role:** HTTP client for the RACFS REST API.

| Item | Kind | Stability | Notes |
|------|------|-----------|--------|
| `Client` | struct | Stable | Main entry point; constructor and method set are the API. |
| `ClientBuilder` | struct | Stable | Builder for pool and timeout options. |
| `Error` | enum | Stable | Client-side errors (network, API, auth). |
| `HealthResponse`, `CapabilitiesResponse` | structs | Stable | Response types for health/capabilities. |
| `FileMetadataResponse`, `DirectoryEntry`, `DirectoryListResponse` | structs | Stable | Align with server JSON; additive fields are backward-compatible. |
| `WriteRequest`, `CreateDirectoryRequest`, `RenameRequest`, etc. | structs | Stable | Request/response types; serde roundtrip with server. |
| `LoginRequest`, `LoginResponse`, `UserResponse` | structs | Stable | Auth API types. |
| `FileQuery`, `XattrQuery`, etc. | structs | Stable | Query types for API calls. |

**Recommendation:** Keep `Client` and `ClientBuilder` stable; ensure new optional fields on request/response types use `Option` or defaults so existing code keeps compiling.

---

## racfs-vfs

**Role:** Virtual filesystem layer with mount routing and plugin types.

| Item | Kind | Stability | Notes |
|------|------|-----------|--------|
| `MountableFS` | struct/trait | Stable | Central VFS; mount/lookup/dispatch. |
| `HandleManager` | struct | Evolving | Handle lifecycle; may grow with new operations. |
| `PluginMetrics` | trait | Stable | Plugins register Prometheus metrics; single method. |
| `plugins::*` | modules | Mixed | Each plugin (MemFS, DevFS, LocalFS, S3FS, etc.) is a public type; plugin constructors and configs are part of the API. |

**Recommendation:** Document which plugins are considered stable (e.g. MemFS, DevFS, LocalFS, ProxyFS, StreamFS, KvFS) vs experimental (e.g. S3FS config, VectorFS, PgsFS). New plugins can be added in minor releases.

---

## racfs-fuse

**Role:** FUSE mount for RACFS servers.

| Item | Kind | Stability | Notes |
|------|------|-----------|--------|
| `mount` | fn | Stable | Read-write mount via blocking adapter (delegates to RacfsAsyncFs). |
| `mount_async` | fn | Stable | Read-only async mount via TokioAdapter(RacfsAsyncFs). |
| `mount_multi` | fn | Stable | Multiple mount points in one process (one thread per mount). |
| `RacfsAsyncFs`, `AsyncFilesystemCompat` | types | Stable | Async implementation; fuser AsyncFilesystem + compat for full ops. |
| `FuseCache` | struct | Evolving | TTL and write-back tuning may change. |
| `DEFAULT_TTL_SECS`, `DEFAULT_WRITE_BACK_*` | constants | Stable | Overridable defaults. |
| `Error` | enum | Stable | Mount and FUSE errors. |
| `InodeManager` | struct | Internal | Exposed for testing; could be made crate-private later. |
| `TokioAdapter` | re-export | Evolving | From fuser; used by mount_async. |
| `advanced::FileLockState` | struct | Stable | Advisory file locking (getlk/setlk). |
| `advanced::MULTI_MOUNT_PLACEHOLDER` | const | Stable | Multi-mount supported (>= 2). |
**Recommendation:** Default mount() is read-write via internal blocking adapter; all logic in RacfsAsyncFs. Use mount_async() for read-only. File locking, libfuse3 feature, multi-mount, Bytes in read path implemented.
---

## racfs-server

**Role:** REST API server and application state; consumed as a library for embedding and tests.

| Item | Kind | Stability | Notes |
|------|------|-----------|--------|
| `ServerConfig` | struct | Stable | TOML config; new optional fields are backward-compatible. |
| `AppState` | struct | Evolving | Request state; may grow with new middleware/features. |
| Auth types (`JwtAuth`, `JwtConfig`, `LoginRequest`, etc.) | types | Stable | Auth API and middleware. |
| `api`, `config`, `error`, `observability`, `state`, `stat_cache`, `validation` | modules | Mixed | Public for embedding and testing; internal details may change. |

**Recommendation:** Treat `ServerConfig` and auth-related exports as stable. The binary (`racfs-server`) is the primary consumer; library users should rely on `ServerConfig` and `AppState` and avoid depending on internal modules where possible.

---

## Deprecated

The following items are marked with `#[deprecated]` in code. See [version-sync.md](version-sync.md) for the deprecation policy (support window, changelog).

| Crate | Item | Replacement |
|-------|------|-------------|
| racfs-server | `StatCache::new` | `StatCache::new_with_ttl` (required when using server config; supports eviction counter and configurable TTL). |

No other deprecations as of this review.

---

## Trait signatures (final)

The following trait method sets are finalized for the 1.0 API. Changing a required method signature or removing a default implementation is a breaking change (major version bump).

### FileSystem (`racfs-core`)

**Required (no default impl):** every backend must implement these.

| Method | Signature (concept) | Notes |
|--------|--------------------|--------|
| `create` | `(path: &Path) -> Result<(), FSError>` | Create empty file. |
| `mkdir` | `(path: &Path, perm: u32) -> Result<(), FSError>` | Create directory. |
| `remove` | `(path: &Path) -> Result<(), FSError>` | Remove file or empty dir. |
| `remove_all` | `(path: &Path) -> Result<(), FSError>` | Remove recursively. |
| `read` | `(path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError>` | size -1 = read to end. |
| `write` | `(path: &Path, data: &[u8], offset: i64, flags: WriteFlags) -> Result<u64, FSError>` | Returns bytes written. |
| `read_dir` | `(path: &Path) -> Result<Vec<FileMetadata>, FSError>` | List directory. |
| `stat` | `(path: &Path) -> Result<FileMetadata, FSError>` | Metadata. |
| `rename` | `(old_path: &Path, new_path: &Path) -> Result<(), FSError>` | Move/rename. |
| `chmod` | `(path: &Path, mode: u32) -> Result<(), FSError>` | Change permissions. |

**Optional (default impl returns `FSError::NotSupported`):** implement only if the backend supports the feature.

| Method | Signature (concept) | Override when |
|--------|--------------------|----------------|
| `truncate` | `(path: &Path, size: u64) -> Result<(), FSError>` | Backend supports truncation. |
| `touch` | `(path: &Path) -> Result<(), FSError>` | Backend supports atime/mtime. |
| `readlink` | `(path: &Path) -> Result<PathBuf, FSError>` | Backend supports symlinks. |
| `symlink` | `(target: &Path, link: &Path) -> Result<(), FSError>` | Backend supports symlinks. |
| `get_xattr`, `set_xattr`, `remove_xattr`, `list_xattr` | path + name/value | Backend supports xattrs. |

### Extension traits (`racfs-core`)

Implement in addition to `FileSystem` when the backend supports the feature.

| Trait | Required methods | Notes |
|-------|------------------|--------|
| `HandleFS` | `open(path, flags) -> Result<FileHandle, FSError>`, `close(handle_id) -> Result<(), FSError>` | Handle-based I/O. |
| `Symlinker` | `symlink`, `readlink` | Override base defaults. |
| `Toucher` | `touch` | Override base default. |
| `Streamer` | `read_stream`, `write_stream` | Streaming read/write. |
| `XattrFS` | `get_xattr`, `set_xattr`, `remove_xattr`, `list_xattr` | Override base defaults. |
| `Linker` | `link(target, link)` | Hard links. |
| `RealPathFS` | `realpath(path) -> Result<PathBuf, FSError>` | Canonical path. |

### Cache (`racfs-core`)

| Method | Required | Default | Notes |
|--------|----------|---------|--------|
| `get(key)` | yes | — | Return `None` if missing/expired. |
| `put(key, value)` | yes | — | Insert or overwrite. |
| `remove(key)` | yes | — | No-op if absent. |
| `invalidate_prefix(prefix)` | no | no-op | Override for prefix invalidation. |
| `stats()` | no | `None` | Override to report hit/miss/eviction. |

### PluginMetrics (`racfs-vfs`)

| Method | Signature | Notes |
|--------|-----------|--------|
| `register` | `(&self, registry: &Registry) -> Result<(), prometheus::Error>` | Register plugin metrics; called once at startup. |

---

## Summary

- **racfs-core** and **racfs-client** form the main long-term contract; their public items should be treated as stable unless explicitly marked deprecated.
- **racfs-vfs** and **racfs-fuse** are stable for their primary entry points (`MountableFS`, `mount`); racfs-fuse uses only AsyncFilesystem (TokioAdapter), read-only until fuser extends its async API.
- **racfs-server** library API is suitable for embedding; `ServerConfig` and auth exports are the stable surface.

## Next steps (ROADMAP)

- [x] Mark deprecated features (add `#[deprecated]` and doc for any identified deprecations).
- [x] Finalize trait signatures (confirm `FileSystem` default methods and any new methods).
- [x] Semantic versioning commitment (document in CHANGELOG / version-sync; adhere to semver for all crates).
