//! Rotating log files filesystem (streamrotatefs) plugin.

mod fs;

pub use fs::{RotateConfig, StreamRotateFS};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, FileSystem, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    #[test]
    fn test_create_fs() {
        let fs = StreamRotateFS::new();
        assert_eq!(fs.config.max_size, 1024 * 1024);
        assert_eq!(fs.config.max_files, 10);
    }

    #[tokio::test]
    async fn test_write_and_read_current() {
        let fs = StreamRotateFS::new();
        let data = b"Hello, World!";

        fs.write(&PathBuf::from("/current"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/current"), 0, -1).await.unwrap();
        assert_eq!(read, data);
    }

    #[tokio::test]
    async fn test_append_current() {
        let fs = StreamRotateFS::new();

        fs.write(
            &PathBuf::from("/current"),
            b"Hello, ",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.write(&PathBuf::from("/current"), b"World!", 0, WriteFlags::none())
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/current"), 0, -1).await.unwrap();
        assert_eq!(read, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_read_dir_root() {
        let fs = StreamRotateFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();

        let paths: Vec<_> = entries
            .iter()
            .map(|e| e.path.to_string_lossy().to_string())
            .collect();
        assert!(paths.contains(&"/current".to_string()));
        assert!(paths.contains(&"/archive".to_string()));
        assert!(paths.contains(&"/rotate".to_string()));
        assert!(paths.contains(&"/config".to_string()));
    }

    #[tokio::test]
    async fn test_manual_rotation() {
        let config = RotateConfig {
            max_size: 1024,
            max_files: 5,
            compress: false,
            base_path: PathBuf::new(),
        };

        let fs = StreamRotateFS::with_config(config.clone());
        fs.write(
            &PathBuf::from("/current"),
            b"Hello, ",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.write(&PathBuf::from("/rotate"), b"rotate", 0, WriteFlags::none())
            .await
            .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/archive")).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .path
                .to_string_lossy()
                .to_string()
                .starts_with("/archive/001.log")
        );

        let current = fs.read(&PathBuf::from("/current"), 0, -1).await.unwrap();
        assert!(current.is_empty());
    }

    #[tokio::test]
    async fn test_auto_rotation() {
        let config = RotateConfig {
            max_size: 10,
            max_files: 3,
            compress: false,
            base_path: PathBuf::new(),
        };

        let fs = StreamRotateFS::with_config(config);

        fs.write(
            &PathBuf::from("/current"),
            b"0123456789",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.write(&PathBuf::from("/current"), b"X", 0, WriteFlags::none())
            .await
            .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/archive")).await.unwrap();
        assert_eq!(entries.len(), 1);

        let current = fs.read(&PathBuf::from("/current"), 0, -1).await.unwrap();
        assert_eq!(current, b"X");
    }

    #[tokio::test]
    async fn test_max_files_pruning() {
        let config = RotateConfig {
            max_size: 5,
            max_files: 3,
            compress: false,
            base_path: PathBuf::new(),
        };

        let fs = StreamRotateFS::with_config(config);

        for i in 0..5 {
            fs.write(&PathBuf::from("/rotate"), b"rotate", 0, WriteFlags::none())
                .await
                .unwrap();
            let current = fs.read(&PathBuf::from("/current"), 0, -1).await.unwrap();
            assert!(
                current.is_empty(),
                "Iteration {}: current should be empty",
                i
            );
        }

        let entries = fs.read_dir(&PathBuf::from("/archive")).await.unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[tokio::test]
    async fn test_config_read() {
        let config = RotateConfig {
            max_size: 2048,
            max_files: 5,
            compress: true,
            base_path: PathBuf::from("/tmp/logs"),
        };

        let fs = StreamRotateFS::with_config(config);
        let config_data = fs.read(&PathBuf::from("/config"), 0, -1).await.unwrap();
        let config_str = String::from_utf8_lossy(&config_data);

        assert!(config_str.contains("max_size=2048"));
        assert!(config_str.contains("max_files=5"));
        assert!(config_str.contains("compress=true"));
        assert!(config_str.contains("/tmp/logs"));
    }

    #[tokio::test]
    async fn test_read_archive_file() {
        let config = RotateConfig {
            max_size: 100,
            max_files: 10,
            compress: false,
            base_path: PathBuf::new(),
        };

        let fs = StreamRotateFS::with_config(config);
        fs.write(
            &PathBuf::from("/current"),
            b"Hello, World!",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.write(&PathBuf::from("/rotate"), b"rotate", 0, WriteFlags::none())
            .await
            .unwrap();

        let archive_data = fs
            .read(&PathBuf::from("/archive/001.log"), 0, -1)
            .await
            .unwrap();
        assert_eq!(archive_data, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_truncate_current() {
        let fs = StreamRotateFS::new();
        fs.write(
            &PathBuf::from("/current"),
            b"Hello, World!",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.truncate(&PathBuf::from("/current"), 5).await.unwrap();

        let current = fs.read(&PathBuf::from("/current"), 0, -1).await.unwrap();
        assert_eq!(current, b"Hello");
    }

    #[tokio::test]
    async fn test_stat() {
        let fs = StreamRotateFS::new();
        fs.write(&PathBuf::from("/current"), b"test", 0, WriteFlags::none())
            .await
            .unwrap();

        let metadata = fs.stat(&PathBuf::from("/current")).await.unwrap();
        assert_eq!(metadata.size, 4);
        assert!(metadata.is_file());

        let dir_meta = fs.stat(&PathBuf::from("/archive")).await.unwrap();
        assert!(dir_meta.is_directory());
    }

    #[tokio::test]
    async fn test_stat_root() {
        let fs = StreamRotateFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.len() >= 4);
    }

    #[tokio::test]
    async fn test_stat_current() {
        let fs = StreamRotateFS::new();
        fs.write(&PathBuf::from("/current"), b"x", 0, WriteFlags::none())
            .await
            .unwrap();
        let meta = fs.stat(&PathBuf::from("/current")).await.unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.size, 1);
    }

    #[tokio::test]
    async fn test_nonexistent_returns_not_found() {
        let fs = StreamRotateFS::new();
        let result = fs.stat(&PathBuf::from("/nonexistent")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }
}
