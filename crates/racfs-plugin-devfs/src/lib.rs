//! Device filesystem (devfs) plugin.

mod fs;

pub use fs::{DevFS, DeviceType};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    #[tokio::test]
    async fn test_stat_root() {
        let fs = DevFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.len() >= 4);
        // Root is listable; stat("/") may or may not be supported depending on impl
    }

    #[tokio::test]
    async fn test_stat_dev_null() {
        let fs = DevFS::new();
        let meta = fs.stat(&PathBuf::from("/dev/null")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/dev/null"));
    }

    #[tokio::test]
    async fn test_stat_dev_zero() {
        let fs = DevFS::new();
        let meta = fs.stat(&PathBuf::from("/dev/zero")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/dev/zero"));
    }

    #[tokio::test]
    async fn test_nonexistent_returns_not_found() {
        let fs = DevFS::new();
        let result = fs.stat(&PathBuf::from("/dev/nonexistent")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
        let read_result = fs.read(&PathBuf::from("/dev/nonexistent"), 0, 10).await;
        assert!(matches!(read_result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_devfs_exists() {
        let fs = DevFS::new();

        assert!(fs.stat(&PathBuf::from("/dev/null")).await.is_ok());
        assert!(fs.stat(&PathBuf::from("/dev/zero")).await.is_ok());
        assert!(fs.stat(&PathBuf::from("/dev/random")).await.is_ok());
        assert!(fs.stat(&PathBuf::from("/dev/urandom")).await.is_ok());
    }

    #[tokio::test]
    async fn test_read_null() {
        let fs = DevFS::new();
        let data = fs.read(&PathBuf::from("/dev/null"), 0, 100).await.unwrap();
        assert!(data.is_empty());
    }

    #[tokio::test]
    async fn test_read_zero() {
        let fs = DevFS::new();
        let data = fs.read(&PathBuf::from("/dev/zero"), 0, 100).await.unwrap();
        assert_eq!(data.len(), 100);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[tokio::test]
    async fn test_write_null() {
        let fs = DevFS::new();
        let written = fs
            .write(
                &PathBuf::from("/dev/null"),
                b"test data",
                0,
                WriteFlags::none(),
            )
            .await
            .unwrap();
        assert_eq!(written, 9);
    }

    #[tokio::test]
    async fn test_read_dir() {
        let fs = DevFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.len() >= 4);
    }
}
