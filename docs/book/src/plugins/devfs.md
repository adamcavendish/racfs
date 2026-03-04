# DevFS

**Device and system information filesystem.** Exposes virtual device files and host info as read-only (or fixed-behavior) files.

## Config

No extra options:

```toml
[mounts.dev]
path = "/dev"
fs_type = "devfs"
```

## Layout and behavior

- **Device files**: `/dev/null`, `/dev/zero`, `/dev/random`, `/dev/urandom` — behave like the real devices (read returns zeros or random bytes; writes to null are dropped).
- **System info**: e.g. hostname, CPU count, exposed as files under the mount.
- Read-only for most paths; writes to `/dev/null` are accepted and discarded.

## Crate

`racfs-plugin-devfs`
