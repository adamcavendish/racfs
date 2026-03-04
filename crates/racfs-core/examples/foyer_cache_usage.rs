//! Use FoyerCache as a bounded in-memory cache implementing the Cache trait.
//!
//! Run: `cargo run -p racfs-core --example foyer_cache_usage`
//!
//! This example demonstrates FoyerCache (foyer in-memory cache) with get/put/remove.
//! Use with CachedFs in racfs-vfs for production read caching when you need
//! bounded capacity and configurable eviction (LRU, LFU, S3Fifo).

use racfs_core::{Cache, FoyerCache};

fn main() {
    let cache = FoyerCache::new(64);

    cache.put("key1", b"value1");
    cache.put("key2", b"value2");

    assert_eq!(cache.get("key1").as_deref(), Some(b"value1" as &[u8]));
    assert_eq!(cache.get("key2").as_deref(), Some(b"value2" as &[u8]));
    assert!(cache.get("missing").is_none());

    cache.remove("key1");
    assert!(cache.get("key1").is_none());
    assert_eq!(cache.get("key2").as_deref(), Some(b"value2" as &[u8]));

    println!("FoyerCache get/put/remove OK.");
}
