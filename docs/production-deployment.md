# Production deployment guide

This guide covers deploying the RACFS server and FUSE clients in production: configuration, monitoring, security, and operations.

## Overview

- **racfs-server** — REST API server that exposes a virtual filesystem (MemFS, LocalFS, S3FS, etc.) over HTTP. Deploy as a binary or Docker container.
- **racfs-fuse** — FUSE client that mounts the server’s namespace locally. Deploy on each host that needs a mount.
- **racfs-cli** — CLI for ad‑hoc access; optional in production.

For architecture and trait system, see [Architecture](architecture.md). For FUSE usage and caching, see [FUSE usage](fuse-usage.md).

---

## 1. Deploying the server

### 1.1 Binary

Build from source (recommended for a specific platform):

```bash
cargo build --release -p racfs-server
```

Binary: `target/release/racfs-server`. Copy to the target host and run:

```bash
./racfs-server [--host 0.0.0.0] [--port 8080] [--config /path/to/config.toml]
```

Or use [release artifacts](https://github.com/adamcavendish/racfs/releases) from the GitHub Release (when available) for Linux/macOS (x64 and aarch64).

### 1.2 Docker

From the repository root:

```bash
docker build -f docker/Dockerfile -t racfs-server:latest .
```

Run with default config (MemFS at `/memfs`, DevFS at `/dev`):

```bash
docker run --rm -p 8080:8080 racfs-server:latest
```

For a custom config, mount a TOML file and set `RACFS_CONFIG`:

```bash
docker run --rm -p 8080:8080 \
  -v /path/on/host/config.toml:/etc/racfs/config.toml:ro \
  -e RACFS_CONFIG=/etc/racfs/config.toml \
  racfs-server:latest
```

See [docker/README.md](../docker/README.md) for more options.

### 1.3 Process management

Run under a process manager so the server restarts on failure and stops cleanly:

- **systemd:** Create a unit file that runs `racfs-server` (or the Docker container) and set `Restart=on-failure`.
- **Kubernetes:** Use a Deployment with a readiness probe pointing at `/health` and a liveness probe; expose the server port via Service.

---

## 2. Configuration

### 2.1 Environment variables

| Variable        | Description                    | Default (if any)   |
|----------------|--------------------------------|--------------------|
| `RACFS_HOST`   | Bind address                   | `127.0.0.1`        |
| `RACFS_PORT`   | Listen port                    | `8080`             |
| `RACFS_CONFIG` | Path to TOML config file       | (none)             |

For production, bind to all interfaces with `RACFS_HOST=0.0.0.0` when the server is not behind a reverse proxy on the same host.

### 2.2 Config file

Use a TOML config file for mounts and optional settings:

```bash
racfs-server --config /etc/racfs/config.toml
# or
RACFS_CONFIG=/etc/racfs/config.toml racfs-server
```

Example layout:

```toml
host = "0.0.0.0"
port = 8080

[mounts.memfs]
type = "memfs"

[mounts.data]
type = "localfs"
root = "/var/racfs/data"

[ratelimit]
max_requests = 200
window_secs = 60

[stat_cache]
ttl_secs = 5
```

See [examples/configs/README.md](../examples/configs/README.md) and `crates/racfs-server/src/config.rs` for the full schema (mount types: memfs, devfs, localfs, proxyfs, streamfs, etc.).

### 2.3 Hot reload (Unix)

When running with a config file, the server handles **SIGHUP** and reloads only **non-critical** settings (no restart):

- **Rate limit** — `[ratelimit]` (e.g. `max_requests`, `window_secs`)
- **Stat cache TTL** — `[stat_cache]` (`ttl_secs`)

Host, port, and mounts are **not** reloaded; change those only by restarting the process.

```bash
kill -HUP <pid>
```

---

## 3. Monitoring and observability

### 3.1 Health and metrics endpoints

- **Health:** `GET /health` — Returns status and version; use for readiness/liveness.
- **Metrics:** `GET /metrics` — Prometheus text format (request duration, cache hits/misses, plugin metrics, etc.).

Example:

```bash
curl -s http://localhost:8080/health
curl -s http://localhost:8080/metrics
```

### 3.2 Grafana

Use the provided dashboard to visualize server metrics:

- **Dashboard JSON:** [dashboards/racfs-server.json](../dashboards/racfs-server.json)
- **Scrape config:** Point Prometheus at `http://<server>:8080/metrics`.

See [dashboards/README.md](../dashboards/README.md) for import steps.

### 3.3 Logging

The server uses **tracing**. Set the subscriber (e.g. env filter) when starting the process, for example:

```bash
RUST_LOG=racfs_server=info,warn racfs-server
```

### 3.4 FUSE client tracing

The FUSE client (`racfs-fuse`) emits **tracing spans** for every FUSE operation (lookup, getattr, read, readdir, create, mkdir, write, unlink, rmdir, rename, chmod, truncate, readlink, symlink, xattr, release, etc.). Each span includes operation-specific fields (e.g. inode, parent, path, offset, size) so you can correlate requests and measure latency in a distributed tracing system.

To enable FUSE tracing:

- **Console (env filter):** run the FUSE binary with a tracing subscriber and an env filter, for example:
  ```bash
  RUST_LOG=racfs_fuse=info ./racfs-fuse /mnt/racfs --server http://racfs:8080
  ```
- **OTLP / Jaeger:** use `tracing_subscriber` with an OTLP or Jaeger layer in your wrapper or by building racfs-fuse with an optional tracing backend; spans will appear as child spans under the process root. No new dependencies were added in racfs-fuse; the crate uses the existing `tracing` crate and `#[instrument]` on all async FUSE operations in `crates/racfs-fuse/src/async_fs.rs`.

---

## 4. Security

### 4.1 Network

- Do not expose the server directly to the internet unless you add authentication and TLS.
- Prefer placing the server behind a reverse proxy (e.g. nginx, Caddy) that terminates TLS and optionally enforces auth.

### 4.2 Authentication

The server supports:

- **JWT** — Configure in config TOML; clients obtain a token via the auth API and send it in `Authorization: Bearer <token>`.
- **API keys** — Clients send `X-API-Key` or `Authorization`; keys are hashed and checked by the server.

See the server’s auth and middleware documentation for setup.

### 4.3 Rate limiting

Rate limiting is enabled by default and can be tuned in config:

```toml
[ratelimit]
max_requests = 200
window_secs = 60
```

Limits are applied per client (e.g. by API key or IP). Reload with SIGHUP.

### 4.4 Input validation

Path, mode, and write body size are validated. Keep the server and dependencies up to date and run `cargo audit` (and any CI checks such as `cargo-deny`) before deployment.

---

## 5. FUSE client deployment

### 5.1 Where to run racfs-fuse

Run **racfs-fuse** on every host that needs a local mount of the RACFS namespace. Each process mounts one directory and talks to one or more RACFS servers (e.g. `--server http://racfs:8080`).

### 5.2 Build and mount

```bash
cargo build --release -p racfs-fuse
mkdir -p /mnt/racfs
./target/release/racfs-fuse /mnt/racfs --server http://racfs:8080
```

For production, run under systemd or another supervisor so the mount is restarted on failure. Ensure the server URL is reachable (DNS, firewall, TLS at the proxy if applicable).

### 5.3 Caching

The FUSE layer caches metadata and readdir with a default TTL (e.g. 1 second). Cache is invalidated on writes. For read‑heavy workloads you can increase the TTL programmatically; see [FUSE usage](fuse-usage.md#caching).

### 5.4 Unmount

- Linux: `fusermount -u /mnt/racfs` or `umount /mnt/racfs`
- macOS: `umount /mnt/racfs`

Graceful shutdown of the FUSE process flushes write‑back buffers and unmounts.

---

## 6. Scaling and operations

- **Single server:** One racfs-server process can serve many FUSE clients and CLI users. Tune `[ratelimit]` and `[stat_cache]` to your load.
- **Multiple servers:** Run several server instances behind a load balancer only if your backends (e.g. MemFS) are shared or replicated; otherwise each instance has its own state.
- **Backups:** For LocalFS or other persistent backends, back up the underlying storage (e.g. the directory or S3 bucket). The REST API does not replace backup tools.
- **Version upgrades:** Follow [Version synchronization](version-sync.md) and the release checklist when upgrading server and SDKs so versions stay aligned.

---

## 7. Quick reference

| Task                 | Command or location |
|----------------------|----------------------|
| Start server         | `racfs-server [--config …]` or Docker (see [docker/README.md](../docker/README.md)) |
| Config examples      | [examples/configs/](../examples/configs/) |
| Health               | `GET /health`        |
| Metrics              | `GET /metrics`       |
| FUSE mount           | `racfs-fuse <dir> --server <url>` |
| Hot reload (Unix)    | `kill -HUP <pid>`    |
| Version/release      | [version-sync.md](version-sync.md), [CHANGELOG.md](../CHANGELOG.md) |
