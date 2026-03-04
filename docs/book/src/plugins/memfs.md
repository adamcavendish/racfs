# MemFS

**In-memory filesystem.** No persistence; all data is lost when the server stops. Useful for caching, temporary scratch space, or tests.

## Config

No extra options. Mount with:

```toml
[mounts.memfs]
path = "/memfs"
fs_type = "memfs"
```

## Behavior

- Full read/write: create, mkdir, read, write, rename, remove, chmod, etc.
- No disk I/O; all state lives in process memory.
- Default mount in the server (with DevFS at `/dev`) when no config is provided.

## Crate

`racfs-plugin-memfs`
