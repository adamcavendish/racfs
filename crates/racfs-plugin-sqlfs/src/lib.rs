//! SQL-backed filesystem (sqlfs) plugin.

mod fs;

pub use fs::{SqlConfig, SqlFS};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    fn create_test_fs() -> SqlFS {
        SqlFS::new().expect("Failed to create test SqlFS")
    }

    #[tokio::test]
    async fn test_create_and_read() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let data = b"Hello, World!";
        fs.write(&PathBuf::from("/file.txt"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/file.txt"), 0, -1).await.unwrap();
        assert_eq!(read, data);
    }

    #[tokio::test]
    async fn test_mkdir_and_read_dir() {
        let fs = create_test_fs();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();

        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/dir")));
    }

    #[tokio::test]
    async fn test_remove() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.remove(&PathBuf::from("/file.txt")).await.unwrap();

        assert!(fs.stat(&PathBuf::from("/file.txt")).await.is_err());
    }

    #[tokio::test]
    async fn test_remove_nonempty_directory() {
        let fs = create_test_fs();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
        fs.create(&PathBuf::from("/dir/file.txt")).await.unwrap();

        assert!(matches!(
            fs.remove(&PathBuf::from("/dir")).await,
            Err(FSError::DirectoryNotEmpty)
        ));

        fs.remove_all(&PathBuf::from("/dir")).await.unwrap();
        assert!(fs.stat(&PathBuf::from("/dir")).await.is_err());
    }

    #[tokio::test]
    async fn test_rename() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/old.txt")).await.unwrap();

        let data = b"test data";
        fs.write(&PathBuf::from("/old.txt"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        fs.rename(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
            .await
            .unwrap();

        assert!(fs.stat(&PathBuf::from("/old.txt")).await.is_err());
        assert!(fs.stat(&PathBuf::from("/new.txt")).await.is_ok());

        let read = fs.read(&PathBuf::from("/new.txt"), 0, -1).await.unwrap();
        assert_eq!(read, data);
    }

    #[tokio::test]
    async fn test_truncate() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let data = b"Hello, World!";
        fs.write(&PathBuf::from("/file.txt"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        fs.truncate(&PathBuf::from("/file.txt"), 5).await.unwrap();

        let read = fs.read(&PathBuf::from("/file.txt"), 0, -1).await.unwrap();
        assert_eq!(read, b"Hello");
    }

    #[tokio::test]
    async fn test_nested_directories() {
        let fs = create_test_fs();
        fs.mkdir(&PathBuf::from("/parent"), 0o755).await.unwrap();
        fs.mkdir(&PathBuf::from("/parent/child"), 0o755)
            .await
            .unwrap();
        fs.create(&PathBuf::from("/parent/child/file.txt"))
            .await
            .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/parent")).await.unwrap();
        assert!(entries
            .iter()
            .any(|e| e.path.as_path() == std::path::Path::new("/parent/child")));
    }

    #[tokio::test]
    async fn test_stat() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let metadata = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(metadata.path, PathBuf::from("/file.txt"));
        assert!(metadata.is_file());
        assert!(!metadata.is_directory());
    }

    #[tokio::test]
    async fn test_chmod() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        fs.chmod(&PathBuf::from("/file.txt"), 0o644).await.unwrap();

        let metadata = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(metadata.permissions(), 0o644);
    }

    #[tokio::test]
    async fn test_touch() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let old_metadata = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;

        fs.touch(&PathBuf::from("/file.txt")).await.unwrap();
        let new_metadata = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();

        assert!(
            new_metadata.modified >= old_metadata.modified,
            "modified time should not be older"
        );
        assert!(
            new_metadata.accessed >= old_metadata.accessed,
            "accessed time should not be older"
        );
    }

    #[tokio::test]
    async fn test_write_with_offset() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        fs.write(&PathBuf::from("/file.txt"), b"Hello", 0, WriteFlags::none())
            .await
            .unwrap();
        fs.write(&PathBuf::from("/file.txt"), b"World", 5, WriteFlags::none())
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/file.txt"), 0, -1).await.unwrap();
        assert_eq!(read, b"HelloWorld");
    }

    #[tokio::test]
    async fn test_write_append() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let flags = WriteFlags::append();
        fs.write(&PathBuf::from("/file.txt"), b"Hello", 0, flags)
            .await
            .unwrap();
        fs.write(&PathBuf::from("/file.txt"), b"World", 0, flags)
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/file.txt"), 0, -1).await.unwrap();
        assert_eq!(read, b"HelloWorld");
    }

    #[tokio::test]
    async fn test_read_with_offset_and_size() {
        let fs = create_test_fs();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let data = b"Hello, World!";
        fs.write(&PathBuf::from("/file.txt"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/file.txt"), 7, 5).await.unwrap();
        assert_eq!(read, b"World");
    }

    #[tokio::test]
    async fn test_create_in_nonexistent_directory_fails() {
        let fs = create_test_fs();

        let result = fs.create(&PathBuf::from("/nonexistent/file.txt")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }
}
