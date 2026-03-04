# LocalFS

**Local disk filesystem.** Uses a directory on the server’s filesystem as the root. All paths under the mount are relative to that root.

## Config

Requires `root` — the absolute path to the directory on disk:

```toml
[mounts.data]
path = "/data"
fs_type = "localfs"
root = "/tmp/racfs-data"
```

Create the directory first, e.g. `mkdir -p /tmp/racfs-data`.

## Behavior

- Standard POSIX-like semantics: create, read, write, mkdir, read_dir, rename, remove, chmod.
- Supports optional per-mount caching (see example configs `cached.toml`).

## Crate

`racfs-plugin-localfs`
