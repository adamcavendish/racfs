//! RACFS CLI - Command-line interface for RACFS

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::fmt;

use racfs_client::Client;

/// Minimum file size (bytes) to show a progress bar for cp.
const CP_PROGRESS_THRESHOLD: u64 = 64 * 1024;

#[derive(Parser)]
#[command(name = "racfs")]
#[command(about = "RACFS - Remote Agent Communication File System CLI", long_about = None)]
struct Cli {
    /// Server URL
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List directory contents
    Ls { path: String },

    /// Read file contents
    Cat { path: String },

    /// Write data to a file
    Write { path: String, data: String },

    /// Create a directory
    Mkdir { path: String },

    /// Remove a file
    Rm { path: String },

    /// Remove a directory
    Rmdir { path: String },

    /// Get file metadata
    Stat { path: String },

    /// Rename/move a file or directory
    Mv { source: String, dest: String },

    /// Copy a file
    Cp { source: String, dest: String },

    /// Recursive directory listing (tree)
    Tree { path: String },

    /// Mount RACFS server at a directory (FUSE)
    Mount {
        /// Mount point path
        path: String,
    },

    /// Change file permissions
    Chmod { path: String, mode: u32 },

    /// Check server health
    Health,

    /// Show server capabilities
    Capabilities,
}

#[tokio::main]
async fn main() {
    fmt::init();

    let cli = Cli::parse();
    let client = Client::new(&cli.server);

    let result = match cli.command {
        Commands::Ls { path } => ls(&client, &path).await,
        Commands::Cat { path } => cat(&client, &path).await,
        Commands::Write { path, data } => write_file(&client, &path, &data).await,
        Commands::Mkdir { path } => mkdir(&client, &path).await,
        Commands::Rm { path } => rm(&client, &path).await,
        Commands::Rmdir { path } => rmdir(&client, &path).await,
        Commands::Stat { path } => stat(&client, &path).await,
        Commands::Mv { source, dest } => mv(&client, &source, &dest).await,
        Commands::Cp { source, dest } => cp(&client, &source, &dest).await,
        Commands::Tree { path } => tree(&client, &path).await,
        Commands::Mount { path } => mount(&cli.server, &path).await,
        Commands::Chmod { path, mode } => chmod(&client, &path, mode).await,
        Commands::Health => health(&client).await,
        Commands::Capabilities => capabilities(&client).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn ls(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.read_dir(path).await?;
    for entry in response.entries {
        println!(
            "{} {:>10} {} {}",
            entry.permissions,
            entry.size,
            entry.modified.unwrap_or_else(|| "unknown".to_string()),
            entry.path
        );
    }
    Ok(())
}

async fn cat(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = client.read_file(path).await?;
    print!("{}", content);
    Ok(())
}

async fn write_file(
    client: &Client,
    path: &str,
    data: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.write_file(path, data, None).await?;
    println!("{}", response.message);
    Ok(())
}

async fn mkdir(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.mkdir(path, None).await?;
    println!("{}", response.message);
    Ok(())
}

async fn rm(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.remove(path).await?;
    println!("{}", response.message);
    Ok(())
}

async fn rmdir(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // For now, rmdir uses remove as well
    let response = client.remove(path).await?;
    println!("{}", response.message);
    Ok(())
}

async fn stat(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.stat(path).await?;
    println!("Type: {}", response.file_type);
    println!("Permissions: {}", response.permissions);
    println!("Size: {}", response.size);
    println!(
        "Modified: {}",
        response.modified.unwrap_or_else(|| "unknown".to_string())
    );
    println!("Path: {}", response.path);
    Ok(())
}

async fn mv(client: &Client, source: &str, dest: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.rename(source, dest).await?;
    println!("{}", response.message);
    Ok(())
}

async fn cp(client: &Client, source: &str, dest: &str) -> Result<(), Box<dyn std::error::Error>> {
    let meta = client.stat(source).await?;
    let size = meta.size;
    let use_progress = size >= CP_PROGRESS_THRESHOLD;

    let pb = if use_progress {
        let bar = ProgressBar::new(size);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("#>-"),
        );
        bar.set_message("Reading...");
        Some(bar)
    } else {
        None
    };

    let content = client.read_file(source).await?;
    if let Some(ref bar) = pb {
        bar.set_position(size / 2);
        bar.set_message("Writing...");
    }

    client.create_file(dest).await?;
    client.write_file(dest, &content, None).await?;

    if let Some(bar) = pb {
        bar.set_position(size);
        bar.finish_with_message("Done");
    } else {
        println!("Copied {} to {} ({} bytes)", source, dest, size);
    }
    Ok(())
}

async fn tree(client: &Client, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", path);
    let mut stack: Vec<(String, String)> = vec![(String::new(), path.to_string())];
    while let Some((prefix, path)) = stack.pop() {
        let list = client.read_dir(&path).await?;
        let entries: Vec<_> = list
            .entries
            .into_iter()
            .filter(|e| {
                let name = e.path.trim_end_matches('/');
                let name = name.rsplit('/').next().unwrap_or(name);
                name != "." && name != ".."
            })
            .collect();
        let mut dirs = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let full_path = entry.path.trim_end_matches('/');
            let name = full_path.rsplit('/').next().unwrap_or(full_path);
            let is_last = i == entries.len() - 1;
            let (branch, next_prefix) = if is_last {
                ("└── ", "    ")
            } else {
                ("├── ", "│   ")
            };
            println!("{}{}{}", prefix, branch, name);
            let meta = match client.stat(full_path).await {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.file_type == "directory" {
                let subpath = if path.ends_with('/') {
                    format!("{}{}", path, name)
                } else if path == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", path, name)
                };
                dirs.push((format!("{}{}", prefix, next_prefix), subpath));
            }
        }
        for item in dirs.into_iter().rev() {
            stack.push(item);
        }
    }
    Ok(())
}

async fn chmod(client: &Client, path: &str, mode: u32) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.chmod(path, mode).await?;
    println!("{}", response.message);
    Ok(())
}

async fn mount(server: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let server = server.to_string();
    let path_buf = PathBuf::from(path);
    println!("Mounting {} at {}", server, path);
    println!(
        "Unmount with: fusermount -u {}  (Linux)  or  umount {}  (macOS)",
        path, path
    );
    tokio::task::spawn_blocking(move || racfs_fuse::mount(&server, path_buf))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.into())
}

async fn health(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.health().await?;
    println!("Status: {}", response.status);
    println!("Version: {}", response.version);
    Ok(())
}

async fn capabilities(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let response = client.capabilities().await?;
    println!("Features:");
    for feature in response.features {
        println!("  - {}", feature);
    }
    println!("Max file size: {} bytes", response.max_file_size);
    Ok(())
}
