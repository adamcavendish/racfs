//! RACFS FUSE - Mount RACFS as a native filesystem

use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::fmt;

#[derive(Parser)]
#[command(name = "racfs-fuse")]
#[command(about = "Mount RACFS as a FUSE filesystem", long_about = None)]
struct Cli {
    /// Mount point
    #[arg()]
    mountpoint: PathBuf,

    /// RACFS server URL
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,
}

fn main() {
    fmt::init();

    let cli = Cli::parse();

    info!("Starting RACFS FUSE mount");
    info!("Server: {}", cli.server);
    info!("Mount point: {:?}", cli.mountpoint);

    if let Err(e) = racfs_fuse::mount(&cli.server, cli.mountpoint) {
        error!("Failed to mount: {}", e);
        std::process::exit(1);
    }
}
