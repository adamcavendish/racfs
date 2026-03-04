# HelloFS

**Simple static read-only demo filesystem.** Exposes a few fixed paths (e.g. `/hello`, `/version`) with static content. Use it to verify the server and FUSE mount without any real storage.

## Layout

- **hello** — Read returns `Hello, World!`.
- **version** — Read returns a version string.

Read-only; create/write/mkdir are not supported (return errors).

## Config

No extra options. Mount for testing:

```toml
[mounts.hello]
path = "/hello"
fs_type = "hellofs"
```

## Crate

`racfs-plugin-hellofs`
