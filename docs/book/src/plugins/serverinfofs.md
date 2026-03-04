# ServerInfoFS

**Server information filesystem.** Read-only files exposing server version, hostname, and other runtime info. Handy for debugging and version checks.

## Layout

- **version** — Server or crate version (e.g. `0.1.0`).
- **hostname** — Hostname of the machine running the server.
- Other files as defined by the plugin.

## Config

No extra options:

```toml
[mounts.info]
path = "/info"
fs_type = "serverinfofs"
```

## Crate

`racfs-plugin-serverinfofs`
