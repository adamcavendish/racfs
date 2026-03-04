//! Programmatic FUSE mount example.
//!
//! Start a RACFS server first: `cargo run -p racfs-server`
//! Then run: `cargo run -p racfs-fuse --example mount -- /tmp/racfs --server http://127.0.0.1:8080`
//! (Unmount with `umount /tmp/racfs` or `fusermount -u /tmp/racfs`.)

use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::fmt;

#[derive(Parser)]
#[command(name = "mount-example")]
struct Args {
    /// Mount point directory
    #[arg(index = 1)]
    mount_point: PathBuf,

    /// RACFS server URL
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,
}

fn main() {
    fmt::init();
    let args = Args::parse();

    println!("Mounting {} at {:?}", args.server, args.mount_point);
    if let Err(e) = racfs_fuse::mount(&args.server, args.mount_point) {
        eprintln!("Mount failed: {}", e);
        std::process::exit(1);
    }
}
