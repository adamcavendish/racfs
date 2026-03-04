# KvFS

**Key-value store as a filesystem.** Keys are paths; values are file contents. In-memory by default; optional SQLite persistence when `database_path` is set.

## Config

- **In-memory (default):** Omit `database_path`. Data is lost when the server stops.

```toml
[mounts.kv]
path = "/kv"
fs_type = "kvfs"
```

- **Persistent (SQLite):** Set `database_path` to a file path. Data survives restarts.

```toml
[mounts.kv]
path = "/kv"
fs_type = "kvfs"
database_path = "./data/kv.db"
```

Create the directory (e.g. `data/`) if it does not exist; the server will create the SQLite file on first use.

## Behavior

- Create a “file” at a path to set a key; read the file to get the value.
- Supports create, read, write, rename, remove, mkdir, read_dir, chmod.
- **In-memory:** Keys are stored in process memory; no persistence.
- **SQLite:** Keys and values are stored in the configured SQLite database; data persists across server restarts.

## Crate

`racfs-plugin-kvfs`
