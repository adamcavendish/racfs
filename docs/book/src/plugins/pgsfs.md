# PgsFS

**PostgreSQL-backed filesystem.** Stores files and directory structure in PostgreSQL. Suited for multi-process or distributed setups where SQL persistence is desired.

## Config

Requires a database connection (e.g. `DATABASE_URL` or config fields for host, database, user, password). See crate and server config for the exact schema.

```toml
[mounts.pgs]
path = "/pgs"
fs_type = "pgsfs"
# ... PostgreSQL connection options
```

## Behavior

- Full `FileSystem` implementation: create, read, write, mkdir, read_dir, rename, remove, chmod.
- Data and metadata are stored in PostgreSQL tables.
- Integration tests require a running Postgres instance; see crate README/tests for how to run them.

## Crate

`racfs-plugin-pgsfs`
