//! HelloFS - A simple static read-only demo filesystem.

mod fs;

pub use fs::HelloFS;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::{error::FSError, flags::WriteFlags};

    use super::*;

    #[tokio::test]
    async fn test_read_hello() {
        let fs = HelloFS::new();
        let data = fs.read(&PathBuf::from("/hello"), 0, -1).await.unwrap();
        assert_eq!(data, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_read_version() {
        let fs = HelloFS::new();
        let data = fs.read(&PathBuf::from("/version"), 0, -1).await.unwrap();
        assert_eq!(data, b"1.0.0");
    }

    #[tokio::test]
    async fn test_read_readme() {
        let fs = HelloFS::new();
        let data = fs.read(&PathBuf::from("/readme.txt"), 0, -1).await.unwrap();
        assert!(data.starts_with(b"HelloFS - A simple static read-only demo filesystem"));
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let fs = HelloFS::new();
        let data = fs.read(&PathBuf::from("/hello"), 7, -1).await.unwrap();
        assert_eq!(data, b"World!");
    }

    #[tokio::test]
    async fn test_read_with_size() {
        let fs = HelloFS::new();
        let data = fs.read(&PathBuf::from("/hello"), 0, 5).await.unwrap();
        assert_eq!(data, b"Hello");
    }

    #[tokio::test]
    async fn test_read_dir() {
        let fs = HelloFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert_eq!(entries.len(), 3);

        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/hello")));
        assert!(paths.contains(&PathBuf::from("/version")));
        assert!(paths.contains(&PathBuf::from("/readme.txt")));
    }

    #[tokio::test]
    async fn test_stat_file() {
        let fs = HelloFS::new();
        let metadata = fs.stat(&PathBuf::from("/hello")).await.unwrap();
        assert_eq!(metadata.path, PathBuf::from("/hello"));
        assert_eq!(metadata.size, 13);
        assert!(metadata.is_file());
    }

    #[tokio::test]
    async fn test_stat_directory() {
        let fs = HelloFS::new();
        let metadata = fs.stat(&PathBuf::from("/")).await.unwrap();
        assert_eq!(metadata.path, PathBuf::from("/"));
        assert!(metadata.is_directory());
    }

    #[tokio::test]
    async fn test_read_directory() {
        let fs = HelloFS::new();
        let result = fs.read(&PathBuf::from("/"), 0, -1).await;
        assert!(matches!(result, Err(FSError::IsDirectory { .. })));
    }

    #[tokio::test]
    async fn test_write_fails() {
        let fs = HelloFS::new();
        let result = fs
            .write(&PathBuf::from("/hello"), b"test", 0, WriteFlags::none())
            .await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_create_fails() {
        let fs = HelloFS::new();
        let result = fs.create(&PathBuf::from("/newfile")).await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_mkdir_fails() {
        let fs = HelloFS::new();
        let result = fs.mkdir(&PathBuf::from("/newdir"), 0o755).await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_remove_fails() {
        let fs = HelloFS::new();
        let result = fs.remove(&PathBuf::from("/hello")).await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_stat_nonexistent() {
        let fs = HelloFS::new();
        let result = fs.stat(&PathBuf::from("/nonexistent")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }
}
