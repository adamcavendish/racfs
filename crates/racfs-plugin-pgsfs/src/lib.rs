//! PostgreSQL-backed filesystem (pgsfs) plugin.

#![allow(clippy::needless_borrows_for_generic_args)]

mod fs;

pub use fs::{PgsConfig, PgsFS};

#[cfg(test)]
mod tests {
    //! PgsFS tests: unit tests run by default; integration tests require PostgreSQL.
    //!
    //! **Run all (unit only):** `cargo test -p racfs-plugin-pgsfs`
    //!
    //! **Run with PostgreSQL:** Start Postgres (e.g. `docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres`),
    //! create DB `racfs_test`, then:
    //! ```text
    //! DATABASE_URL=postgres://postgres:postgres@localhost/racfs_test cargo test -p racfs-plugin-pgsfs -- --ignored
    //! ```
    //! Set `SKIP_PGS_TESTS=1` to make integration tests no-op when run with `--ignored`.

    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{ChmodFS, DirFS, FileSystem, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    fn create_test_fs() -> Result<PgsFS, String> {
        if std::env::var("SKIP_PGS_TESTS").is_ok() {
            return Err("SKIP_PGS_TESTS set".into());
        }

        let config = PgsConfig {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost/racfs_test".to_string()
            }),
            max_connections: 5,
            max_file_size: 5 * 1024 * 1024,
            min_idle_connections: Some(1),
        };

        PgsFS::with_config(config).map_err(|e| e.to_string())
    }

    #[tokio::test]
    async fn test_pgs_config_default() {
        let config = PgsConfig::default();
        assert!(!config.database_url.is_empty());
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.max_file_size, 5 * 1024 * 1024);
        assert_eq!(config.min_idle_connections, Some(2));
    }

    #[tokio::test]
    #[ignore] // Requires PostgreSQL running
    async fn test_create_and_read() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let data = b"Hello, World!";
        fs.write(&PathBuf::from("/file.txt"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let read = fs.read(&PathBuf::from("/file.txt"), 0, -1).await.unwrap();
        assert_eq!(read, data);
    }

    #[tokio::test]
    #[ignore]
    async fn test_mkdir_and_read_dir() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
        fs.mkdir(&PathBuf::from("/dir"), 0o755).await.unwrap();

        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/dir")));
    }

    #[tokio::test]
    #[ignore]
    async fn test_remove() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();
        fs.remove(&PathBuf::from("/file.txt")).await.unwrap();

        assert!(fs.stat(&PathBuf::from("/file.txt")).await.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_remove_nonempty_directory() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
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
    #[ignore]
    async fn test_rename() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
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
    #[ignore]
    async fn test_truncate() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
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
    #[ignore]
    async fn test_stat() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        let metadata = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(metadata.path, PathBuf::from("/file.txt"));
        assert!(metadata.is_file());
        assert!(!metadata.is_directory());
    }

    #[tokio::test]
    #[ignore]
    async fn test_chmod() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
        fs.create(&PathBuf::from("/file.txt")).await.unwrap();

        fs.chmod(&PathBuf::from("/file.txt"), 0o644).await.unwrap();

        let metadata = fs.stat(&PathBuf::from("/file.txt")).await.unwrap();
        assert_eq!(metadata.permissions(), 0o644);
    }

    #[tokio::test]
    #[ignore]
    async fn test_write_with_offset() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
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
    #[ignore]
    async fn test_write_exceeds_max_file_size_returns_storage_full() {
        let fs = match create_test_fs() {
            Ok(f) => f,
            Err(_) => return,
        };
        fs.create(&PathBuf::from("/big.txt")).await.unwrap();

        let oversized = vec![0u8; 6 * 1024 * 1024];
        let result = fs
            .write(
                &PathBuf::from("/big.txt"),
                &oversized,
                0,
                WriteFlags::none(),
            )
            .await;

        assert!(
            matches!(result, Err(FSError::StorageFull)),
            "expected StorageFull, got {:?}",
            result
        );
    }
}
