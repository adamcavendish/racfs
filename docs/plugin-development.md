# Plugin Development Tutorial

This guide explains how to build a custom RACFS plugin by implementing the `FileSystem` trait. For architecture context, see [Architecture](architecture.md).

## Overview

A RACFS plugin is any type that implements `racfs_core::FileSystem`. The server and FUSE layer call into this trait; they do not depend on your storage backend. You can implement in-memory trees, database-backed storage, or remote APIs.

## Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
racfs-core = { path = "../racfs-core" }  # or from crates.io when published
async-trait = "0.1"
```

## The FileSystem Trait

`FileSystem` is built from four subtraits. Implement the subtraits so each group of operations can live in a separate impl (or file):

| Subtrait | Methods | Purpose |
|----------|---------|---------|
| `ReadFS` | `read`, `stat` | Read file content and metadata |
| `WriteFS` | `create`, `write` | Create files and write data |
| `DirFS` | `mkdir`, `read_dir`, `remove`, `remove_all`, `rename` | Directories and paths |
| `ChmodFS` | `chmod` | Change permissions |

`FileSystem` extends `ReadFS + WriteFS + DirFS + ChmodFS + Send + Sync` and adds default methods for optional operations: `truncate`, `touch`, `readlink`, `symlink`, `get_xattr`, `set_xattr`, `remove_xattr`, `list_xattr`. Override those in `impl FileSystem for YourType` only when your backend supports them.

Import:

```rust
use racfs_core::filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};
```

Implement all four subtraits, then add `#[async_trait] impl FileSystem for YourType {}` (or override only the default methods you support). The trait is async and requires `Send + Sync`. Use `#[async_trait]` when implementing.

## Errors

Return `Result<T, FSError>`. Common variants:

- `FSError::NotFound { path }` — path does not exist
- `FSError::ReadOnly` — for read-only filesystems (create, write, mkdir, remove, rename, chmod)
- `FSError::AlreadyExists { path }` — create/mkdir on existing path
- `FSError::NotADirectory { path }` / `FSError::NotAFile { path }` — type mismatch

## Metadata

Use helpers from `racfs_core::metadata::FileMetadata`:

- `FileMetadata::file(path, size)` — regular file
- `FileMetadata::directory(path)` — directory
- `FileMetadata::symlink(path, target)` — symlink
- `FileMetadata::new(path, mode)` — custom mode (see `S_IFREG`, `S_IFDIR`, etc. in `racfs_core::metadata`)

Paths are `PathBuf`; use `Path::new("/")` and `PathBuf::from("/hello")` for comparisons and returns.

## Minimal Read-Only Example

A read-only plugin implements the four subtraits and an empty `FileSystem` impl:

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{
    error::FSError,
    filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS},
    flags::WriteFlags,
    metadata::FileMetadata,
};

struct MyPlugin {
    content: Vec<u8>,
}

#[async_trait]
impl ReadFS for MyPlugin {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        if path != Path::new("/hello") {
            return Err(FSError::NotFound { path: path.to_path_buf() });
        }
        let start = offset.max(0) as usize;
        let end = if size < 0 {
            self.content.len()
        } else {
            (start + size as usize).min(self.content.len())
        };
        if start >= self.content.len() {
            return Ok(Vec::new());
        }
        Ok(self.content[start..end].to_vec())
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        if path == Path::new("/") {
            Ok(FileMetadata::directory(PathBuf::from("/")))
        } else if path == Path::new("/hello") {
            Ok(FileMetadata::file(PathBuf::from("/hello"), self.content.len() as u64))
        } else {
            Err(FSError::NotFound { path: path.to_path_buf() })
        }
    }
}

#[async_trait]
impl WriteFS for MyPlugin {
    async fn create(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn write(&self, _path: &Path, _data: &[u8], _offset: i64, _flags: WriteFlags) -> Result<u64, FSError> {
        Err(FSError::ReadOnly)
    }
}

#[async_trait]
impl DirFS for MyPlugin {
    async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        if path != Path::new("/") {
            return Err(FSError::NotFound { path: path.to_path_buf() });
        }
        Ok(vec![
            FileMetadata::directory(PathBuf::from("/")),
            FileMetadata::file(PathBuf::from("/hello"), self.content.len() as u64),
        ])
    }

    async fn remove(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn remove_all(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn rename(&self, _old: &Path, _new: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}

#[async_trait]
impl ChmodFS for MyPlugin {
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}

#[async_trait]
impl FileSystem for MyPlugin {}
```

## Running the Example

The repository includes a full example at `crates/racfs-vfs/examples/custom_plugin.rs`. Run it:

```bash
cargo run -p racfs-vfs --example custom_plugin
```

## Serving Your Plugin

To expose the plugin over the REST API and FUSE:

1. **Mount in the VFS** — The server uses `racfs_vfs::MountableFS`, which mounts plugins at paths (e.g. `/memfs`). To add your plugin, register it in the server’s mount table (see server config or state) so that a path like `/myplugin/...` is forwarded to your implementation.

2. **Standalone** — You can also drive your plugin directly in tests or tools by calling `plugin.stat()`, `plugin.read()`, etc., as in the custom_plugin example.

## Next Steps

- Browse existing plugins in `crates/racfs-vfs/src/plugins/` (e.g. `memfs.rs`, `localfs.rs`) for patterns.
- See [FUSE usage](fuse-usage.md) for mounting a running server via FUSE.
- See [Architecture](architecture.md) for request flow and conventions.
