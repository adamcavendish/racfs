//! Streaming data example using the StreamFS plugin.
//!
//! StreamFS exposes streams as a virtual filesystem:
//!   /streams/{name}/     - stream directory
//!   /streams/{name}/tail - write here to append messages
//!   /streams/{name}/head - next message ID to read
//!   /streams/{name}/data/ - message files (000001.msg, 000002.msg, ...)
//!
//! Run: `cargo run -p racfs-vfs --example streaming_data`

use std::path::Path;

use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
use racfs_core::flags::WriteFlags;
use racfs_plugin_streamfs::{StreamConfig, StreamFS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamConfig {
        buffer_size: 100,
        history_size: 50,
        max_streams: 10,
        compression: None,
    };
    let fs = StreamFS::new(config);

    let stream_name = "events";
    let stream_path = format!("/streams/{}", stream_name);
    let data_path = format!("/streams/{}/data", stream_name);
    let tail_path = format!("/streams/{}/tail", stream_name);
    let head_path = format!("/streams/{}/head", stream_name);
    let config_path = format!("/streams/{}/config", stream_name);

    println!("1. Create stream '{}'", stream_name);
    fs.mkdir(Path::new(&stream_path), 0o755).await?;

    println!("2. Append messages by writing to tail");
    for i in 1..=3 {
        let msg = format!("{{ \"event\": \"msg{}\", \"ts\": {} }}\n", i, i * 1000);
        let n = fs
            .write(Path::new(&tail_path), msg.as_bytes(), 0, WriteFlags::none())
            .await?;
        println!("   Wrote {} bytes (message {})", n, i);
    }

    println!("3. Read head and tail (read positions)");
    let head = fs.read(Path::new(&head_path), 0, 20).await?;
    let tail = fs.read(Path::new(&tail_path), 0, 20).await?;
    println!(
        "   head = {}  tail = {}",
        String::from_utf8_lossy(&head),
        String::from_utf8_lossy(&tail)
    );

    println!("4. Read stream config");
    let cfg = fs.read(Path::new(&config_path), 0, 256).await?;
    println!("   {}", String::from_utf8_lossy(&cfg));

    println!("5. List message files in data/");
    let entries: Vec<_> = fs.read_dir(Path::new(&data_path)).await?;
    for e in &entries {
        println!("   {} (size {})", e.path.display(), e.size);
    }

    println!("6. Read first message");
    let first_msg = fs
        .read(Path::new("/streams/events/data/000001.msg"), 0, -1)
        .await?;
    println!("   {}", String::from_utf8_lossy(&first_msg));

    println!("7. Remove stream (cleanup)");
    fs.remove(Path::new(&stream_path)).await?;

    println!("\nDone. To use StreamFS via the server, add to config:");
    println!("  [mounts.events]");
    println!("  path = \"/streams\"");
    println!("  fs_type = \"streamfs\"");
    Ok(())
}
