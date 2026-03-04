# RACFS Implementation Quick Reference

**Last Updated:** 2026-03-08

---

## Project Overview

RACFS is a virtual filesystem with pluggable backends, REST API, FUSE mount support, and multi-language SDKs.

**Current Version:** v0.1.0
**Target Version:** v1.0.0 (4-6 months)
**Active Milestone:** v0.2.0 - FUSE Foundation

---

## Quick Links

- **Roadmap:** `/Volumes/files/repo/adamcavendish/racfs/ROADMAP.md`
- **v0.2.0 Tasks:** `/Volumes/files/repo/adamcavendish/racfs/docs/v0.2.0-tasks.md`
- **Project Instructions:** `/Volumes/files/repo/adamcavendish/racfs/CLAUDE.md`

---

## Workspace Structure

```
racfs/
├── crates/
│   ├── racfs-core/          # Core traits, errors, types
│   ├── racfs-vfs/           # 16 filesystem plugins
│   ├── racfs-fuse/          # FUSE mount (v0.2.0 focus)
│   ├── racfs-server/        # REST API server
│   ├── racfs-client/        # HTTP client
│   └── racfs-cli/           # CLI tool
├── sdks/
│   ├── python/              # Python SDK
│   └── typescript/          # TypeScript SDK
├── docs/                    # Documentation
├── examples/                # Example code
└── ROADMAP.md              # This roadmap
```

---

## Common Commands

### Build & Test
```bash
# Build all crates
cargo build --workspace

# Build specific crate
cargo build -p racfs-fuse

# Test all crates
cargo test --workspace

# Test specific crate
cargo test -p racfs-fuse

# Run clippy
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt
```

### Run Server
```bash
# Start server (default port 8080)
cargo run -p racfs-server

# Start server with custom config
cargo run -p racfs-server -- --config config.toml
```

### Run CLI
```bash
# List files
cargo run -p racfs-cli -- ls /memfs

# Read file
cargo run -p racfs-cli -- cat /memfs/test.txt

# Write file
cargo run -p racfs-cli -- write /memfs/test.txt "content"

# Create directory
cargo run -p racfs-cli -- mkdir /memfs/testdir
```

### FUSE Mount (v0.2.0+)
```bash
# Build FUSE binary
cargo build --release -p racfs-fuse

# Mount filesystem
./target/release/racfs-fuse /tmp/racfs --server http://localhost:8080

# Unmount (Linux)
fusermount -u /tmp/racfs

# Unmount (macOS)
umount /tmp/racfs
```

---

## Key Patterns

### Error Handling
- Use `snafu` for error handling
- Define errors in `FSError` enum
- Use context for error messages

```rust
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum FSError {
    #[snafu(display("File not found: {}", path))]
    NotFound { path: String },
}
```

### Logging
- Use `tracing` for structured logging
- Use appropriate log levels (trace, debug, info, warn, error)

```rust
use tracing::{info, debug, error};

info!("Starting FUSE mount at {}", mount_point);
debug!("Allocated inode {} for path {}", ino, path);
error!("Failed to read file: {}", err);
```

### Async Operations
- Use `tokio` for async runtime
- Use `async/await` for async operations

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RacfsClient::new("http://localhost:8080").await?;
    let data = client.read_file("/memfs/test.txt").await?;
    Ok(())
}
```

---

## FileSystem Trait

All VFS backends implement the `FileSystem` trait from `racfs-core`. `FileSystem` is composed of four subtraits; call sites that use methods like `read`, `stat`, `create`, `write`, `mkdir`, `read_dir`, `remove`, `rename`, `chmod` should bring the appropriate trait into scope:

- **ReadFS**: `read`, `stat`
- **WriteFS**: `create`, `write`
- **DirFS**: `mkdir`, `read_dir`, `remove`, `remove_all`, `rename`
- **ChmodFS**: `chmod`

```rust
use racfs_core::filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};

// Use vfs.read(), vfs.stat(), vfs.create(), etc. with trait in scope
```

---

## FUSE Implementation Notes (v0.2.0)

### Inode Management
- Root inode is always 1
- Allocate unique inodes for each path
- Use HashMap or DashMap for thread-safety
- Clean up inodes on file deletion

### Error Mapping
Map `FSError` to FUSE error codes:
- `FSError::NotFound` → `libc::ENOENT`
- `FSError::PermissionDenied` → `libc::EACCES`
- `FSError::AlreadyExists` → `libc::EEXIST`
- `FSError::NotADirectory` → `libc::ENOTDIR`
- `FSError::IsADirectory` → `libc::EISDIR`

### Caching Strategy
- Metadata cache with TTL (default 5s)
- Read-ahead for sequential reads
- Write-back for small writes
- Invalidate on writes

---

## Testing Strategy

### Unit Tests
- Test individual functions
- Mock dependencies
- Use `#[cfg(test)]` modules

### Integration Tests
- Test end-to-end workflows
- Use real server and client
- Test error scenarios

### POSIX Compliance Tests
- Test all POSIX operations
- Test edge cases
- Test concurrent access

---

## Current Status (v0.2.0)

**Progress:** 20% (1/5 tasks complete)

**Completed:**
- ✅ Create roadmap document
- ✅ Create task breakdown
- ✅ Project setup (dependencies, modules)
- ✅ Inode manager implementation
- ✅ FUSE struct with tokio runtime
- ✅ Read-only operations (lookup, getattr, readdir, read)
- ✅ Mount/unmount functionality

**In Progress:**
- 🔄 Researching AsyncFilesystem trait migration

**Next Steps:**
1. ⬜ Migrate to AsyncFilesystem trait (recommended)
2. ⬜ Implement write operations
3. ⬜ Implement advanced operations
4. ⬜ Add client-side caching
5. ⬜ Create integration tests

**Blockers:** None

**Technical Debt:**
- Current implementation uses `runtime.block_on()` which blocks async runtime
- Should migrate to `AsyncFilesystem` trait for better async support
- Need proper file handle management for write operations

---

## AsyncFilesystem Migration (v0.2.1)

**Status:** 📋 Planned
**Priority:** P1 (High)
**Effort:** 1 week

The fuser crate provides experimental async support that's better suited for RACFS:

### Benefits
- ✅ Native async/await without `block_on()`
- ✅ Better performance with proper async task spawning
- ✅ Built-in response types (`LookupResponse`, `GetAttrResponse`)
- ✅ Cleaner error handling with `Result<T, Errno>`

### Key Components
- `fuser::experimental::AsyncFilesystem` - Async trait
- `fuser::experimental::TokioAdapter` - Bridges async to sync FUSE
- `fuser::experimental::RequestContext` - Request context
- `fuser::experimental::DirEntListBuilder` - Directory listing builder

### Example
```rust
use fuser::experimental::{AsyncFilesystem, TokioAdapter};

#[async_trait::async_trait]
impl AsyncFilesystem for RacfsAsyncFs {
    async fn lookup(&self, context: &RequestContext, parent: INodeNo, name: &OsStr)
        -> Result<LookupResponse, Errno>
    {
        // Native async - no block_on!
        let metadata = self.client.stat(&path).await?;
        Ok(LookupResponse::new(TTL, attr, Generation(0)))
    }
}

// In mount function:
let fs = RacfsAsyncFs::new(server_url)?;
let adapter = TokioAdapter::new(fs);
fuser::mount2(adapter, &mount_point, &config)?;
```

See [docs/async-filesystem-migration.md](async-filesystem-migration.md) for full migration guide.

---

## Milestones Timeline

| Milestone | Start | End | Duration |
|-----------|-------|-----|----------|
| v0.2.0 - FUSE Foundation | 2026-03-08 | 2026-04-19 | 6 weeks |
| v0.3.0 - Performance | 2026-04-19 | 2026-05-17 | 4 weeks |
| v0.4.0 - DevEx | 2026-03-08 | 2026-03-29 | 3 weeks (parallel) |
| v0.5.0 - Production | 2026-05-17 | 2026-06-14 | 4 weeks |
| v1.0.0 - GA | 2026-06-14 | 2026-07-26 | 6 weeks |

**Total Timeline:** 16-23 weeks (4-6 months)

---

## Resources

### Documentation
- [FUSE Documentation](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
- [fuser Crate](https://docs.rs/fuser/)
- [Tokio Documentation](https://tokio.rs/)
- [Snafu Documentation](https://docs.rs/snafu/)

### Similar Projects
- [s3fs-fuse](https://github.com/s3fs-fuse/s3fs-fuse)
- [rclone](https://rclone.org/)
- [sshfs](https://github.com/libfuse/sshfs)

---

## Contact & Support

- **Issues:** GitHub Issues
- **Discussions:** GitHub Discussions
- **Documentation:** `docs/` directory

---

## Notes

- FUSE implementation is the highest priority (v0.2.0)
- Developer experience work can run in parallel (v0.4.0)
- Performance optimization comes after basic FUSE works (v0.3.0)
- Production readiness requires performance metrics (v0.5.0)
- v1.0.0 requires all previous milestones complete
