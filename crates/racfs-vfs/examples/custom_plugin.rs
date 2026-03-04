//! Minimal custom plugin implementing the FileSystem trait.
//!
//! Run: `cargo run -p racfs-vfs --example custom_plugin`
//!
//! This example implements a read-only filesystem with a root directory
//! and a single file `/hello` containing "Hello from custom plugin!".

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use racfs_core::{
    error::FSError,
    filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS},
    flags::WriteFlags,
    metadata::FileMetadata,
};

/// Minimal read-only plugin: root (/) and one file (/hello).
struct CustomPlugin {
    hello_content: Vec<u8>,
}

impl CustomPlugin {
    fn new() -> Self {
        Self {
            hello_content: b"Hello from custom plugin!".to_vec(),
        }
    }
}

#[async_trait]
impl ReadFS for CustomPlugin {
    async fn read(&self, path: &Path, offset: i64, size: i64) -> Result<Vec<u8>, FSError> {
        if path == Path::new("/hello") {
            let start = offset.max(0) as usize;
            let end = if size < 0 {
                self.hello_content.len()
            } else {
                (start + size as usize).min(self.hello_content.len())
            };
            if start >= self.hello_content.len() {
                return Ok(Vec::new());
            }
            Ok(self.hello_content[start..end].to_vec())
        } else {
            Err(FSError::NotFound {
                path: path.to_path_buf(),
            })
        }
    }

    async fn stat(&self, path: &Path) -> Result<FileMetadata, FSError> {
        if path == Path::new("/") {
            Ok(FileMetadata::directory(PathBuf::from("/")))
        } else if path == Path::new("/hello") {
            Ok(FileMetadata::file(
                PathBuf::from("/hello"),
                self.hello_content.len() as u64,
            ))
        } else {
            Err(FSError::NotFound {
                path: path.to_path_buf(),
            })
        }
    }
}

#[async_trait]
impl WriteFS for CustomPlugin {
    async fn create(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn write(
        &self,
        _path: &Path,
        _data: &[u8],
        _offset: i64,
        _flags: WriteFlags,
    ) -> Result<u64, FSError> {
        Err(FSError::ReadOnly)
    }
}

#[async_trait]
impl DirFS for CustomPlugin {
    async fn mkdir(&self, _path: &Path, _perm: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn read_dir(&self, path: &Path) -> Result<Vec<FileMetadata>, FSError> {
        if path == Path::new("/") {
            Ok(vec![
                FileMetadata::directory(PathBuf::from("/")),
                FileMetadata::file(PathBuf::from("/hello"), 26),
            ])
        } else {
            Err(FSError::NotFound {
                path: path.to_path_buf(),
            })
        }
    }

    async fn remove(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn remove_all(&self, _path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }

    async fn rename(&self, _old_path: &Path, _new_path: &Path) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}

#[async_trait]
impl ChmodFS for CustomPlugin {
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<(), FSError> {
        Err(FSError::ReadOnly)
    }
}

#[async_trait]
impl FileSystem for CustomPlugin {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plugin = CustomPlugin::new();

    println!("Custom plugin: stat /");
    let meta = plugin.stat(Path::new("/")).await?;
    println!("  {:?}", meta.path);
    assert!(meta.is_directory());

    println!("Custom plugin: stat /hello");
    let meta = plugin.stat(Path::new("/hello")).await?;
    println!("  size = {}", meta.size);

    println!("Custom plugin: read /hello");
    let data = plugin.read(Path::new("/hello"), 0, -1).await?;
    let s = String::from_utf8(data)?;
    println!("  {}", s);

    println!("Custom plugin: read_dir /");
    let entries = plugin.read_dir(Path::new("/")).await?;
    for e in entries {
        println!("  {} {}", e.file_type(), e.path.display());
    }

    println!("\nDone. Use MountableFS::mount() to serve this plugin behind a path.");
    Ok(())
}
