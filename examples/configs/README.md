# RACFS configuration examples

Example TOML configs for the RACFS server. Use with `--config` or `RACFS_CONFIG`:

```bash
# From repo root
cargo run -p racfs-server -- --config examples/configs/default.toml

# Or with env
RACFS_CONFIG=examples/configs/localfs.toml cargo run -p racfs-server
```

## Files

| File | Description |
|------|-------------|
| **default.toml** | MemFS at `/memfs`, DevFS at `/dev` (same as no config). |
| **localfs.toml** | Adds LocalFS at `/data` with root `/tmp/racfs-data`. Create the directory first: `mkdir -p /tmp/racfs-data`. |
| **streamfs.toml** | Adds StreamFS at `/streams` for streaming data (buffer_size, history_size, max_streams). |
| **proxyfs.toml** | Adds ProxyFS at `/remote` proxying to `http://localhost:9090`. Start another server on 9090 to test. |
| **kvfs.toml** | Adds KvFS at `/kv` – in-memory key-value store as a filesystem (create, read, write, rename). |
| **kvfs_persistent.toml** | KvFS at `/kv` with `database_path = "./data/kv.db"` – data persists across restarts. Run `mkdir -p data` first. |
| **vectorfs.toml** | Adds VectorFS at `/vectors` – document store with vector embeddings and search. Optional `db_path`, `embedding_url`. |
| **queuefs.toml** | Adds QueueFS at `/queues` – message queues as filesystem (head, tail, .ack, etc.). |
| **cached.toml** | Same as default plus a mount with optional `[cache]` (enabled, ttl_secs, max_entries). Used when plugins support per-mount caching. |

## Overriding with env

Host and port can be overridden without editing the file:

```bash
RACFS_HOST=0.0.0.0 RACFS_PORT=9000 cargo run -p racfs-server -- --config examples/configs/default.toml
```

## Mount options reference

- **memfs** – In-memory filesystem. No extra options.
- **devfs** – Device info (hostname, cpus, etc.). No extra options.
- **localfs** – Local directory. Requires `root = "/path/to/dir"`.
- **proxyfs** – Proxy to another API. Requires `url = "http://..."`.
- **streamfs** – Streaming streams. Optional: `buffer_size`, `history_size`, `max_streams`.
- **kvfs** – Key-value store as filesystem. Optional: `database_path` (SQLite file for persistence); when omitted, in-memory only.
- **vectorfs** – Document store with vector embeddings and similarity search. Optional: `db_path` (persistence), `embedding_url` (embedding API URL).
- **queuefs** – Message queues as filesystem. No extra options.

**Per-mount cache (optional):** Any mount can include a `[mounts.<name>.cache]` section for plugins that support caching (e.g. future Foyer-backed backends):

```toml
[mounts.mydata]
path = "/data"
fs_type = "localfs"
root = "/tmp/data"
[mounts.mydata.cache]
enabled = true
ttl_secs = 60
max_entries = 1000
```

- `enabled` – Turn on caching for this mount (default: false).
- `ttl_secs` – Optional TTL in seconds.
- `max_entries` – Optional cap on cached entries.

When `enabled = true`, `ttl_secs` and `max_entries` must be > 0 if set. See `cached.toml` for a full example.

See `crates/racfs-server/src/config.rs` for the full `MountConfig` schema.

## Hot reload (Unix)

When running with a config file (`--config` or `RACFS_CONFIG`), the server listens for **SIGHUP**. Sending SIGHUP re-reads the config file and applies **non-critical** settings without restart:

- **Rate limit** – `[ratelimit]` with `max_requests` and `window_secs`
- **Stat cache TTL** – `[stat_cache]` with `ttl_secs`

Example optional sections (defaults if omitted):

```toml
[ratelimit]
max_requests = 200
window_secs = 60

[stat_cache]
ttl_secs = 5
```

Host, port, and mounts are **not** reloaded (restart required).
