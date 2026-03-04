# Plugins overview

RACFS exposes storage through **plugins**. Each plugin implements the same `FileSystem` trait and is mounted at a path (e.g. `/memfs`, `/data`, `/kv`). The server config file (or defaults) defines which plugins are loaded and where they appear.

## Built-in plugins

| Plugin | Crate | Description |
|--------|--------|-------------|
| **MemFS** | racfs-plugin-memfs | In-memory filesystem; no persistence. |
| **LocalFS** | racfs-plugin-localfs | Local disk; requires a root directory. |
| **DevFS** | racfs-plugin-devfs | Device files: `/dev/null`, `/dev/zero`, hostname, CPU count, etc. |
| **KvFS** | racfs-plugin-kvfs | Key-value store exposed as a filesystem. |
| **SQLfs** | racfs-plugin-sqlfs | SQLite-backed filesystem. |
| **HttpFS** | racfs-plugin-httpfs | HTTP client: write requests to files, read responses. |
| **S3FS** | racfs-plugin-s3fs | S3-compatible object storage (multipart uploads). |
| **VectorFS** | racfs-plugin-vectorfs | Document store with vector embeddings and similarity search. |
| **QueueFS** | racfs-plugin-queuefs | Message queues as directories; head, tail, ack. |
| **StreamFS** | racfs-plugin-streamfs | Streaming data with optional compression. |
| **StreamRotateFS** | racfs-plugin-streamrotatefs | Rotating log files (size- or count-based). |
| **ProxyFS** | racfs-plugin-proxyfs | Proxies requests to another RACFS server. |
| **HeartbeatFS** | racfs-plugin-heartbeatfs | Health monitoring: status, uptime, heartbeat counter. |
| **ServerInfoFS** | racfs-plugin-serverinfofs | Server info: version, hostname, etc. |
| **HelloFS** | racfs-plugin-hellofs | Simple read-only demo: `/hello`, `/version`. |
| **PgsFS** | racfs-plugin-pgsfs | PostgreSQL-backed filesystem. |

## Configuring plugins

In the server config (e.g. `examples/configs/default.toml`), each mount has a path and a plugin type:

```toml
[mounts.memfs]
path = "/memfs"
fs_type = "memfs"

[mounts.data]
path = "/data"
fs_type = "localfs"
root = "/tmp/racfs-data"
```

See [examples/configs/README.md](https://github.com/adamcavendish/racfs/blob/master/examples/configs/README.md) for all example configs and mount options.

## Plugin pages

Each plugin has a short chapter in this book with an introduction, layout or behavior, and config options where applicable. Use the sidebar or the summary to jump to a plugin.
