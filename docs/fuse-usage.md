# FUSE Usage Guide

This guide explains how to build, mount, and use the RACFS FUSE filesystem.

## Prerequisites

- **Linux:** FUSE libraries and kernel support. Install with `sudo apt install libfuse-dev` (Debian/Ubuntu) or equivalent. Your user must be in the `fuse` group to mount without root.
- **macOS:** [macFUSE](https://osxfuse.github.io/) must be installed. The mount point must exist and be empty.

## Build

```bash
cargo build --release -p racfs-fuse
```

To link against **libfuse3** for mount/umount (Linux):  
`cargo build --release -p racfs-fuse --features libfuse3`

Binary: `target/release/racfs-fuse` (or `racfs-fuse.exe` on Windows; FUSE is typically used on Linux/macOS).

## Start the Server

In a separate terminal:

```bash
cargo run -p racfs-server
```

Default URL: `http://127.0.0.1:8080`. The server mounts MemFS at `/memfs` by default.

## Mount

```bash
# Create mount point
mkdir -p /tmp/racfs

# Mount (blocks until unmount)
./target/release/racfs-fuse /tmp/racfs --server http://127.0.0.1:8080
```

Or run in the background:

```bash
./target/release/racfs-fuse /tmp/racfs --server http://127.0.0.1:8080 &
```

## Use the Mount

The default **mount()** is **read-write**: you can create files, write, mkdir, unlink, rename, etc. Use normal shell commands:

```bash
ls /tmp/racfs
ls /tmp/racfs/memfs
echo "hello" > /tmp/racfs/memfs/hello.txt
cat /tmp/racfs/memfs/hello.txt
mkdir /tmp/racfs/memfs/mydir
mv /tmp/racfs/memfs/hello.txt /tmp/racfs/memfs/mydir/
```

For **read-only** async mounting (no blocking), use **mount_async()** from the library; the CLI binary uses **mount()** (read-write).

**File locking:** Advisory POSIX locks (flock, lockf) are supported process-locally via getlk/setlk.

**mmap:** Memory-mapped reads are supported (getattr and read are coherent with the kernel page cache). For write-back of mapped writes, use `msync()`; behavior is consistent with the FUSE cache and server.

## Unmount

- **Linux:** `fusermount -u /tmp/racfs` or `umount /tmp/racfs`
- **macOS:** `umount /tmp/racfs`

If the FUSE process is in the foreground, you can also stop it with Ctrl+C (the mount will be released).

## Programmatic Mount

Use the library from your own binary:

```rust
use racfs_fuse::mount;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = "http://127.0.0.1:8080";
    let mount_point = PathBuf::from("/tmp/racfs");
    mount(server, mount_point)?;
    Ok(())
}
```

Or run the example:

```bash
cargo run -p racfs-fuse --example mount -- /tmp/racfs --server http://127.0.0.1:8080
```

## Caching

The FUSE layer caches metadata (stat) and directory listings with a default TTL of 1 second. Cache is invalidated on create, mkdir, write, unlink, rmdir, and rename. To use a custom TTL:

```rust
use racfs_fuse::{RacfsAsyncFs, FuseCache};
use std::time::Duration;

let cache = FuseCache::new(Duration::from_secs(5));
let fs = RacfsAsyncFs::with_cache("http://127.0.0.1:8080", cache)?;
// Build mount with fuser's TokioAdapter and mount2 (see lib.rs mount())...
```

## Troubleshooting

- **Permission denied (Linux):** Ensure your user is in the `fuse` group: `sudo usermod -aG fuse $USER` (log out and back in).
- **Device not found / mount point not found:** Create the directory first and ensure it is empty.
- **Connection refused:** Start the RACFS server before mounting, and use the correct URL and port.
- **Stale cache:** After modifying the filesystem from another client (e.g. CLI), wait for the TTL (default 1s) or unmount and remount to see changes.

## Verification

See the **Verification Commands** section in [ROADMAP.md](../ROADMAP.md) for a full list of test commands.
