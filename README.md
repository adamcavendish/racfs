# RACFS (Remote Agent Call File System)

A virtual filesystem with FUSE mount, REST API server, and client in Rust.

**For detailed documentation** — including introductions to every plugin, configuration, and usage — see the **[RACFS Book](docs/book/README.md)**. From the repo root you can run:

```bash
just book        # build the book (output in docs/book/build/)
just book-serve  # serve at http://localhost:3000 with live reload
```

Or install mdbook and run it from `docs/book/` yourself.

## Overview

RACFS provides a virtual filesystem abstraction with pluggable backends:

- **Plugins**: memfs, localfs, devfs, kvfs, sqlfs, httpfs, s3fs, vectorfs, queuefs, streamfs, proxyfs, and more
- **FUSE mount**: Mount virtual filesystems as local directories (async `AsyncFilesystem` + TokioAdapter)
- **REST API**: HTTP server with OpenAPI docs, config-based mounts, rate limiting, auth
- **Client**: HTTP client with retry, circuit breaker, connection pooling

## Project Structure

```
racfs/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── racfs-core/         # Core traits, errors, types, cache, compression
│   ├── racfs-http-error/   # HTTP error codes and response types
│   ├── racfs-vfs/          # Virtual filesystem plugins
│   ├── racfs-fuse/         # FUSE mount implementation
│   ├── racfs-server/       # REST API server (axum)
│   ├── racfs-client/       # HTTP client
│   └── racfs-cli/          # CLI tool
├── docs/                   # Architecture, guides, API review
├── examples/configs/       # Server config examples
└── ROADMAP.md              # Milestones and Stratara requirements
```

## Plugins

| Plugin     | Description                    |
|-----------|--------------------------------|
| memfs     | In-memory filesystem           |
| localfs   | Local disk filesystem          |
| devfs     | Device files (/dev/null, etc.) |
| kvfs      | Key-value store                |
| sqlfs     | SQLite-backed                  |
| httpfs    | HTTP request/response          |
| s3fs      | AWS S3 (multipart uploads)     |
| vectorfs  | Vector search + embeddings     |
| queuefs   | Message queue                  |
| streamfs  | Streaming + optional compression |
| proxyfs   | Proxy to another RACFS server  |
| heartbeatfs, serverinfofs, hellofs | Monitoring and demo |

## Quick Start

### Build and test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt
```

### Run server

```bash
cargo run -p racfs-server
```

Server listens on `http://localhost:8080`. OpenAPI UI: `/swagger-ui`.

With a config file:

```bash
RACFS_CONFIG=examples/configs/default.toml cargo run -p racfs-server
```

### FUSE mount

```bash
cargo build --release -p racfs-fuse
./target/release/racfs-fuse /tmp/racfs --server http://localhost:8080
# Then: ls /tmp/racfs, cat, mkdir, etc.
# Unmount: fusermount -u /tmp/racfs  (Linux) or umount /tmp/racfs (macOS)
```

### Coverage

```bash
cargo install cargo-tarpaulin
just coverage          # stdout
just coverage-html     # coverage/
```

## REST API

- **Files**: GET/POST/PUT/DELETE `/api/v1/files`
- **Directories**: GET/POST `/api/v1/directories`
- **Metadata**: GET `/api/v1/stat`, POST `/api/v1/rename`, POST `/api/v1/chmod`, etc.
- **Docs**: GET `/swagger-ui`

## Documentation

- [Architecture](docs/architecture.md)
- [FUSE usage](docs/fuse-usage.md)
- [Plugin development](docs/plugin-development.md)
- [Production deployment](docs/production-deployment.md)
- [ROADMAP](ROADMAP.md) — milestones and downstream (Stratara) requirements

## License

See LICENSE file.
