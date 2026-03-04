//! Use CachedFs to wrap any FileSystem with a cache.
//!
//! Run: `cargo run -p racfs-vfs --example cached_fs_usage`
//!
//! This example wraps MemFS with CachedFs using HashMapCache. For production
//! use a bounded cache: use `FoyerCache::new(max_entries)` instead of HashMapCache.

use std::path::Path;
use std::sync::Arc;

use racfs_core::{
    Cache, HashMapCache,
    filesystem::{ReadFS, WriteFS},
    flags::WriteFlags,
};
use racfs_plugin_memfs::MemFS;
use racfs_vfs::CachedFs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inner = Arc::new(MemFS::new());
    inner.create(Path::new("/file.txt")).await?;
    inner
        .write(
            Path::new("/file.txt"),
            b"Hello, cached world!",
            0,
            WriteFlags::none(),
        )
        .await?;

    let cache = Arc::new(HashMapCache::new());
    let cached = CachedFs::new(inner.clone(), cache.clone());

    println!("First read (miss):");
    let data = cached.read(Path::new("/file.txt"), 0, -1).await?;
    println!("  {} bytes", data.len());
    assert!(cache.get("read:/file.txt").is_some());

    println!("Second read (hit):");
    let data2 = cached.read(Path::new("/file.txt"), 0, -1).await?;
    assert_eq!(data, data2);

    println!("Stat (cached after first call):");
    let _ = cached.stat(Path::new("/file.txt")).await?;
    assert!(cache.get("stat:/file.txt").is_some());

    println!("With key prefix (e.g. per-mount):");
    let cache2 = Arc::new(HashMapCache::new());
    let cached_mount = CachedFs::new(inner, cache2.clone()).with_key_prefix("/mount/");
    let _ = cached_mount.read(Path::new("/file.txt"), 0, -1).await?;
    assert!(cache2.get("/mount/read:/file.txt").is_some());

    println!("Done.");
    Ok(())
}
