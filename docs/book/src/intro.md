# Introduction

**RACFS** (Remote Agent Call File System) is a virtual filesystem implemented in Rust. It provides:

- **Pluggable backends** — Storage is implemented by plugins (MemFS, LocalFS, S3FS, and more). Each plugin implements the same `FileSystem` trait.
- **REST API** — The server exposes the filesystem over HTTP. You can read and write files via standard HTTP endpoints.
- **FUSE mount** — Mount any RACFS server as a local directory using the FUSE layer. The kernel talks to your RFS server over HTTP.
- **Client and CLI** — Use the HTTP client from your own code, or the `racfs` CLI for copy, tree, mount, and other operations.

This book focuses on **plugins**: what each plugin does, how to configure it, and typical use cases. For high-level architecture, server configuration, FUSE usage, and plugin development, see the [More documentation](SUMMARY.md#more-documentation) section at the end of the summary.

## Quick start

Build and test:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Run the server (default: MemFS at `/memfs`, DevFS at `/dev`):

```bash
cargo run -p racfs-server
```

Server listens on `http://localhost:8080`. OpenAPI UI: `/swagger-ui`.

Mount via FUSE:

```bash
cargo build --release -p racfs-fuse
./target/release/racfs-fuse /tmp/racfs --server http://localhost:8080
# Use /tmp/racfs like a normal directory; unmount with fusermount -u /tmp/racfs (Linux) or umount /tmp/racfs (macOS)
```

Use a config file to mount more plugins:

```bash
RACFS_CONFIG=examples/configs/default.toml cargo run -p racfs-server
```

See [examples/configs/README.md](../../examples/configs/README.md) for available configs (localfs, kvfs, vectorfs, queuefs, etc.).

## Reading this book

- **[Plugins overview](plugins/overview.md)** — Summary of all built-in plugins and how they are mounted.
- **Individual plugin pages** — One page per plugin with a short introduction, config options, and layout or usage notes.

To build and serve this book locally:

```bash
cargo install mdbook
cd docs/book && mdbook build
# Or serve with live reload:
cd docs/book && mdbook serve --open
```

The generated HTML is in `docs/book/build/`.
