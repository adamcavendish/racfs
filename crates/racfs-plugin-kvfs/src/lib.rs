//! Key-Value store filesystem (kvfs) plugin.

mod fs;

pub use fs::KvFS;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{ChmodFS, DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;
    use crate::fs::KvBackend;

    #[tokio::test]
    async fn test_new() {
        let fs = KvFS::new();
        match &fs.backend {
            KvBackend::Memory(store, metadata) => {
                assert!(store.read().is_empty());
                assert!(metadata.read().is_empty());
            }
            _ => panic!("expected Memory backend"),
        }
    }

    #[tokio::test]
    async fn test_default() {
        let fs = KvFS::default();
        match &fs.backend {
            KvBackend::Memory(store, _) => assert!(store.read().is_empty()),
            _ => panic!("expected Memory backend"),
        }
    }

    #[tokio::test]
    async fn test_with_data() {
        let mut data = std::collections::HashMap::new();
        data.insert(PathBuf::from("/test.txt"), b"hello".to_vec());

        let fs = KvFS::with_data(data);
        let content = fs.read(&PathBuf::from("/test.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"hello");
    }

    #[tokio::test]
    async fn test_create_and_read() {
        let fs = KvFS::new();

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
    }

    #[tokio::test]
    async fn test_write_with_offset() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();
        fs.write(&PathBuf::from("/test.txt"), b"Hello", 0, WriteFlags::none())
            .await
            .unwrap();

        fs.write(
            &PathBuf::from("/test.txt"),
            b" World",
            5,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let content = fs.read(&PathBuf::from("/test.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"Hello World");
    }

    #[tokio::test]
    async fn test_write_append() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();
        fs.write(&PathBuf::from("/test.txt"), b"Hello", 0, WriteFlags::none())
            .await
            .unwrap();

        fs.write(
            &PathBuf::from("/test.txt"),
            b" World",
            0,
            WriteFlags::APPEND,
        )
        .await
        .unwrap();

        let content = fs.read(&PathBuf::from("/test.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"Hello World");
    }

    #[tokio::test]
    async fn test_read_with_offset_and_size() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/test.txt"),
            b"Hello, World!",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let content = fs.read(&PathBuf::from("/test.txt"), 7, 5).await.unwrap();
        assert_eq!(content, b"World");
    }

    #[tokio::test]
    async fn test_remove() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();
        fs.write(&PathBuf::from("/test.txt"), b"test", 0, WriteFlags::none())
            .await
            .unwrap();

        fs.remove(&PathBuf::from("/test.txt")).await.unwrap();

        let result = fs.read(&PathBuf::from("/test.txt"), 0, -1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_all() {
        let fs = KvFS::new();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
        fs.create(&PathBuf::from("/dir/file1.txt")).await.unwrap();
        fs.create(&PathBuf::from("/dir/file2.txt")).await.unwrap();

        fs.remove_all(&PathBuf::from("/dir")).await.unwrap();

        assert!(fs.stat(&PathBuf::from("/dir/file1.txt")).await.is_err());
        assert!(fs.stat(&PathBuf::from("/dir/file2.txt")).await.is_err());
    }

    #[tokio::test]
    async fn test_mkdir() {
        let fs = KvFS::new();
        fs.mkdir(&PathBuf::from("/mydir"), 0o755).await.unwrap();

        let meta = fs.stat(&PathBuf::from("/mydir")).await.unwrap();
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_rename() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/old.txt")).await.unwrap();
        fs.write(&PathBuf::from("/old.txt"), b"data", 0, WriteFlags::none())
            .await
            .unwrap();

        fs.rename(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
            .await
            .unwrap();

        assert!(fs.stat(&PathBuf::from("/old.txt")).await.is_err());

        let content = fs.read(&PathBuf::from("/new.txt"), 0, -1).await.unwrap();
        assert_eq!(content, b"data");
    }

    #[tokio::test]
    async fn test_chmod() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();

        fs.chmod(&PathBuf::from("/test.txt"), 0o644).await.unwrap();

        let meta = fs.stat(&PathBuf::from("/test.txt")).await.unwrap();
        assert_eq!(meta.mode & 0o777, 0o644);
    }

    #[tokio::test]
    async fn test_stat_not_found() {
        let fs = KvFS::new();
        let result = fs.stat(&PathBuf::from("/nonexistent.txt")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_create_already_exists() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/test.txt")).await.unwrap();

        let result = fs.create(&PathBuf::from("/test.txt")).await;
        assert!(matches!(result, Err(FSError::AlreadyExists { .. })));
    }

    #[tokio::test]
    async fn test_write_not_found() {
        let fs = KvFS::new();
        let result = fs
            .write(
                &PathBuf::from("/nonexistent.txt"),
                b"data",
                0,
                WriteFlags::none(),
            )
            .await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_rename_to_existing() {
        let fs = KvFS::new();
        fs.create(&PathBuf::from("/old.txt")).await.unwrap();
        fs.create(&PathBuf::from("/new.txt")).await.unwrap();

        let result = fs
            .rename(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
            .await;
        assert!(matches!(result, Err(FSError::AlreadyExists { .. })));
    }

    #[tokio::test]
    async fn test_read_dir() {
        let fs = KvFS::new();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
        fs.create(&PathBuf::from("/dir/file1.txt")).await.unwrap();
        fs.create(&PathBuf::from("/dir/file2.txt")).await.unwrap();

        let entries = fs.read_dir(&PathBuf::from("/dir")).await.unwrap();
        assert_eq!(entries.len(), 2);

        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/dir/file1.txt")));
        assert!(paths.contains(&PathBuf::from("/dir/file2.txt")));
    }

    #[tokio::test]
    async fn test_persistence_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("kv.db");

        {
            let fs = KvFS::with_database(&db_path).unwrap();
            fs.create(&PathBuf::from("/persist.txt")).await.unwrap();
            fs.write(
                &PathBuf::from("/persist.txt"),
                b"persisted data",
                0,
                WriteFlags::none(),
            )
            .await
            .unwrap();
        }

        let fs = KvFS::with_database(&db_path).unwrap();
        let content = fs
            .read(&PathBuf::from("/persist.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(content, b"persisted data");
    }

    #[tokio::test]
    async fn test_persistence_mkdir_read_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("kv.db");

        {
            let fs = KvFS::with_database(&db_path).unwrap();
            fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
            fs.create(&PathBuf::from("/dir/file.txt")).await.unwrap();
            fs.write(
                &PathBuf::from("/dir/file.txt"),
                b"hello",
                0,
                WriteFlags::none(),
            )
            .await
            .unwrap();
        }

        let fs = KvFS::with_database(&db_path).unwrap();
        let entries = fs.read_dir(&PathBuf::from("/dir")).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, PathBuf::from("/dir/file.txt"));
        let content = fs
            .read(&PathBuf::from("/dir/file.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(content, b"hello");
    }

    #[tokio::test]
    async fn test_persistence_chmod_after_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("kv.db");

        {
            let fs = KvFS::with_database(&db_path).unwrap();
            fs.create(&PathBuf::from("/chmod.txt")).await.unwrap();
            fs.chmod(&PathBuf::from("/chmod.txt"), 0o600).await.unwrap();
        }

        let fs = KvFS::with_database(&db_path).unwrap();
        let meta = fs.stat(&PathBuf::from("/chmod.txt")).await.unwrap();
        assert_eq!(meta.mode & 0o777, 0o600);
    }
}
