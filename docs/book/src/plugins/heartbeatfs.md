# HeartbeatFS

**Health monitoring filesystem.** Exposes health and liveness info as read-only files: status, uptime, heartbeat counter. Useful for load balancers and readiness probes.

## Layout

- **status** — Read returns a simple status string (e.g. `ok`).
- **uptime** — Uptime or similar metric.
- Other files may expose a heartbeat counter or timestamps.

## Config

No extra options:

```toml
[mounts.heartbeat]
path = "/heartbeat"
fs_type = "heartbeatfs"
```

## Crate

`racfs-plugin-heartbeatfs`
