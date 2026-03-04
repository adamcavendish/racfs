//! In-memory filesystem (memfs) plugin.

mod fs;

pub use fs::{MemFS, MemFSMetrics};

// Re-export PluginMetrics so consumers can bound MemFS without depending on racfs-vfs directly
// for the trait.
pub use racfs_vfs::PluginMetrics;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;
    use racfs_vfs::PluginMetrics;

    use super::*;

    #[tokio::test]
    async fn test_create_and_read() {
        let fs = MemFS::new();
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
        let fs = MemFS::new();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();

        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        eprintln!("entries: {:?}", entries);
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/dir")));
    }

    #[tokio::test]
    async fn test_remove() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.remove(&PathBuf::from("/file.txt")).await.unwrap();

        assert!(fs.stat(&PathBuf::from("/file.txt")).await.is_err());
    }

    #[tokio::test]
    async fn test_plugin_metrics_register_and_inc() {
        use prometheus::{Encoder, Registry, TextEncoder};

        let fs = MemFS::new();
        let registry = Registry::new();
        fs.register(&registry).unwrap();
        let metrics = registry.gather();
        assert!(
            !metrics.is_empty(),
            "expected at least one plugin metric family"
        );
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        encoder.encode(&metrics, &mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();
        assert!(
            output.contains("racfs_plugin_memfs"),
            "expected memfs metric prefix"
        );

        fs.create(&PathBuf::from("/metric_test.txt")).await.unwrap();
        let mut buffer2 = Vec::new();
        encoder.encode(&registry.gather(), &mut buffer2).unwrap();
        let output2 = String::from_utf8(buffer2).unwrap();
        assert!(output2.contains("racfs_plugin_memfs_operations_total"));
        assert!(output2.contains("create"));
    }

    #[tokio::test]
    async fn test_symlink_and_readlink() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/file.txt"),
            b"content",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.symlink(&PathBuf::from("/file.txt"), &PathBuf::from("/link"))
            .await
            .unwrap();

        let meta = fs.stat(&PathBuf::from("/link")).await.unwrap();
        assert!(meta.is_symlink());
        assert_eq!(
            meta.symlink_target.as_ref().unwrap(),
            &PathBuf::from("/file.txt")
        );

        let target = fs.readlink(&PathBuf::from("/link")).await.unwrap();
        assert_eq!(target, PathBuf::from("/file.txt"));

        let content = fs.read(&PathBuf::from("/link"), 0, -1).await.unwrap();
        assert_eq!(content, b"content");
    }

    #[tokio::test]
    async fn test_xattr() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        fs.set_xattr(&PathBuf::from("/file.txt"), "user.foo", b"bar")
            .await
            .unwrap();
        fs.set_xattr(&PathBuf::from("/file.txt"), "user.baz", b"quux")
            .await
            .unwrap();

        let list = fs.list_xattr(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"user.foo".to_string()));
        assert!(list.contains(&"user.baz".to_string()));

        let v = fs
            .get_xattr(&PathBuf::from("/file.txt"), "user.foo")
            .await
            .unwrap();
        assert_eq!(v, b"bar");

        fs.remove_xattr(&PathBuf::from("/file.txt"), "user.foo")
            .await
            .unwrap();
        let list2 = fs.list_xattr(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(list2.len(), 1);
        assert!(
            fs.get_xattr(&PathBuf::from("/file.txt"), "user.foo")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_stat_root() {
        let fs = MemFS::new();
        let meta = fs.stat(&PathBuf::from("/")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/"));
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_stat_file_after_write() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/file.txt"),
            b"Hello, World!",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let meta = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/file.txt"));
        assert!(meta.is_file());
        assert_eq!(meta.size, 13);
    }

    #[tokio::test]
    async fn test_rename() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/old.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/old.txt"),
            b"content",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.rename(&PathBuf::from("/old.txt"), &PathBuf::from("/new.txt"))
            .await
            .unwrap();

        assert!(fs.stat(&PathBuf::from("/old.txt")).await.is_err());
        let data = fs.read(&PathBuf::from("/new.txt"), 0, -1).await.unwrap();
        assert_eq!(data, b"content");
    }

    #[tokio::test]
    async fn test_remove_all() {
        let fs = MemFS::new();
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();
        fs.create(&PathBuf::from("/dir/file.txt")).await.unwrap();
        fs.write(&PathBuf::from("/dir/file.txt"), b"x", 0, WriteFlags::none())
            .await
            .unwrap();

        fs.remove_all(&PathBuf::from("/dir")).await.unwrap();

        assert!(fs.stat(&PathBuf::from("/dir")).await.is_err());
        assert!(fs.stat(&PathBuf::from("/dir/file.txt")).await.is_err());
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(!entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/dir")));
    }

    #[tokio::test]
    async fn test_chmod() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.chmod(&PathBuf::from("/file.txt"), 0o600).await.unwrap();

        let meta = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(meta.permissions(), 0o600);
    }

    #[tokio::test]
    async fn test_create_on_existing_returns_already_exists() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let result = fs.create(&PathBuf::from("/file.txt")).await;
        assert!(matches!(result, Err(FSError::AlreadyExists { .. })));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_size() {
        let fs = MemFS::new();
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/file.txt"),
            b"0123456789",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let data = fs.read(&PathBuf::from("/file.txt"), 2, 3).await.unwrap();
        assert_eq!(data, b"234");
    }

    #[tokio::test]
    async fn test_write_with_offset() {
        let fs = MemFS::new();
        let p = PathBuf::from("/file.txt");
        fs.create(&p).await.unwrap();
        fs.write(&p, b"abc", 0, WriteFlags::none()).await.unwrap();
        fs.write(&p, b"XY", 1, WriteFlags::none()).await.unwrap();

        let data = fs.read(&p, 0, -1).await.unwrap();
        assert_eq!(
            data.len(),
            3,
            "write at offset should extend/replace in place"
        );
        assert_eq!(data, b"aXY");
    }
}
