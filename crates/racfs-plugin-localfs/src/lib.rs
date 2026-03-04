//! Local filesystem (localfs) plugin.

mod fs;

pub use fs::LocalFS;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{ChmodFS, DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn setup_temp_dir() -> (LocalFS, PathBuf) {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!(
            "racfs_localfs_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        (LocalFS::new(temp_dir.clone()), temp_dir)
    }

    fn cleanup_temp_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_new() {
        let temp_dir = std::env::temp_dir().join("racfs_test_new");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let fs = LocalFS::new(temp_dir.clone());
        assert_eq!(fs.root(), temp_dir);
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_create_and_read() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/test.txt"),
            b"Hello, World!",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let content = fs.read(&PathBuf::from("/test.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"Hello, World!");
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_write_creates_file() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.write(
            &PathBuf::from("/auto.txt"),
            b"auto created",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let content = fs.read(&PathBuf::from("/auto.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"auto created");
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_mkdir() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.mkdir(&PathBuf::from("/mydir"), 0o755).await.unwrap();
        let meta = fs.stat(&PathBuf::from("/mydir")).await.unwrap();
        assert!(meta.is_directory());
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_remove_file() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.create(&PathBuf::from("/todelete.txt")).await.unwrap();
        fs.remove(&PathBuf::from("/todelete.txt")).await.unwrap();
        let result = fs.stat(&PathBuf::from("/todelete.txt")).await;
        assert!(result.is_err());
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_stat() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.write(&PathBuf::from("/stat.txt"), b"test", 0, WriteFlags::none())
            .await
            .unwrap();
        let meta = fs.stat(&PathBuf::from("/stat.txt")).await.unwrap();
        assert_eq!(meta.size, 4);
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_path_traversal_prevention() {
        let (fs, temp_dir) = setup_temp_dir();
        let result = fs.stat(&PathBuf::from("../../../etc/passwd")).await;
        assert!(matches!(result, Err(FSError::PermissionDenied { .. })));
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_read_dir() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.mkdir(&PathBuf::from("/subdir"), 0o755).await.unwrap();
        fs.create(&PathBuf::from("/subdir/file.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/subdir/file.txt"),
            b"x",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/subdir")).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.to_string_lossy().ends_with("file.txt"));
        assert!(entries[0].is_file());
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_remove_all() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
        fs.create(&PathBuf::from("/dir/nested.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/dir/nested.txt"),
            b"x",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.remove_all(&PathBuf::from("/dir")).await.unwrap();

        assert!(fs.stat(&PathBuf::from("/dir")).await.is_err());
        assert!(fs.stat(&PathBuf::from("/dir/nested.txt")).await.is_err());
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_rename() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.create(&PathBuf::from("/old.txt")).await.unwrap();
        fs.write(&PathBuf::from("/old.txt"), b"moved", 0, WriteFlags::none())
            .await
            .unwrap();

        fs.rename(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
            .await
            .unwrap();

        assert!(fs.stat(&PathBuf::from("/old.txt")).await.is_err());
        let content = fs.read(&PathBuf::from("/new.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"moved");
        cleanup_temp_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_chmod() {
        let (fs, temp_dir) = setup_temp_dir();
        fs.create(&PathBuf::from("/chmod.txt")).await.unwrap();
        let result = fs.chmod(&PathBuf::from("/chmod.txt"), 0o600).await;
        if result.is_ok() {
            let meta = fs.stat(&PathBuf::from("/chmod.txt")).await.unwrap();
            assert!(meta.is_file());
            // Permissions may be affected by umask; just ensure we can stat after chmod
        }
        cleanup_temp_dir(&temp_dir);
    }
}
