# RACFS Architecture

This document describes the high-level architecture, crate boundaries, and key design decisions.

## Overview

RACFS is a virtual filesystem with pluggable backends exposed via a REST API and mountable with FUSE. Clients can use the HTTP API directly or mount the filesystem locally.

```
┌─────────────────┐     HTTP      ┌──────────────────┐     trait      ┌─────────────────┐
│  racfs-fuse     │ ◄───────────► │  racfs-server    │ ◄────────────► │  racfs-vfs      │
│  (FUSE mount)   │               │  (REST API)      │                │  (plugins)      │
└────────┬────────┘               └────────┬─────────┘                └────────┬────────┘
         │                                 │                                  │
         │                                 │                                  │
         ▼                                 ▼                                  ▼
┌─────────────────┐               ┌──────────────────┐               ┌─────────────────┐
│  racfs-client    │               │  racfs-core       │               │  MemFS, S3FS,    │
│  (HTTP client)   │               │  (FileSystem      │               │  HttpFS, etc.    │
└─────────────────┘               │   trait, errors)  │               └─────────────────┘
                                  └──────────────────┘
```

## Crates

| Crate | Role |
|-------|------|
| **racfs-core** | Defines the `FileSystem` trait, `FSError`, `FileMetadata`, and shared types. All backends implement this trait. |
| **racfs-vfs** | Virtual filesystem layer: `MountableFS` routes paths to mounted plugins (MemFS, S3FS, HttpFS, etc.). |
| **racfs-server** | Axum REST API server. Holds a `MountableFS`, maps HTTP endpoints to `FileSystem` calls. |
| **racfs-client** | HTTP client for the REST API. Used by the CLI and by racfs-fuse. |
| **racfs-fuse** | FUSE implementation. Wraps the client, implements `fuser::Filesystem`, provides TTL cache. |
| **racfs-cli** | Command-line tool (ls, cat, write, mkdir, rm, etc.) using racfs-client. |

## Trait System

All storage backends implement `racfs_core::FileSystem`:

- **Required:** `create`, `mkdir`, `remove`, `remove_all`, `read`, `write`, `read_dir`, `stat`, `rename`, `chmod`
- **Optional (defaults):** `truncate`, `touch`
- **Extensions:** `HandleFS`, `Symlinker`, `Toucher`, `XattrFS`, `Linker`, `RealPathFS` (optional)

The server and FUSE layer only depend on the core trait, so new plugins (e.g. database-backed) can be added without changing the server or FUSE code.

## Request Flow

### REST API

1. Client sends HTTP request (e.g. `GET /api/v1/files?path=/memfs/foo`).
2. Server resolves path via `MountableFS`: `/memfs/foo` → plugin at `/memfs` (e.g. MemFS), path `foo`.
3. Server calls `plugin.read(&path).await` and returns the response.

### FUSE Mount

1. Kernel sends FUSE request (e.g. lookup, getattr, read).
2. racfs-fuse maps (inode/path) to logical path, checks local TTL cache.
3. On cache miss, racfs-fuse uses racfs-client to call the same REST API.
4. Responses are cached and returned to the kernel.

## Conventions

- **Paths:** Server and VFS use absolute paths. Mounted plugins see paths relative to their mount point.
- **Errors:** `FSError` in core; mapped to HTTP status and FUSE `Errno` at the boundaries.
- **Async:** All filesystem operations are async; FUSE uses a sync wrapper with `block_on` (or future AsyncFilesystem + TokioAdapter).

## Configuration

- Server: config file (e.g. `config.toml`) and/or environment; defines mount points and plugin config.
  - **Environment variables:** `RACFS_CONFIG` – path to config file; `RACFS_HOST` – bind address; `RACFS_PORT` – port. Env overrides file values.
- FUSE: CLI args (mount point, server URL); cache TTL is configurable via `RacfsAsyncFs::with_cache`.
