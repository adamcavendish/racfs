//! Server information filesystem (serverinfofs) plugin.

mod fs;

pub use fs::ServerInfoFS;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    #[tokio::test]
    async fn test_serverinfofs_creation() {
        let fs = ServerInfoFS::new();
        assert!(fs.stat(&PathBuf::from("/")).await.is_ok());
    }

    #[tokio::test]
    async fn test_read_version() {
        let fs = ServerInfoFS::new();
        let data = fs.read(&PathBuf::from("/version"), 0, -1).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&data), "0.1.0");
    }

    #[tokio::test]
    async fn test_read_hostname() {
        let fs = ServerInfoFS::new();
        let data = fs.read(&PathBuf::from("/hostname"), 0, -1).await.unwrap();
        assert!(!data.is_empty());
    }

    #[tokio::test]
    async fn test_read_cpu_count() {
        let fs = ServerInfoFS::new();
        let data = fs.read(&PathBuf::from("/cpu/count"), 0, -1).await.unwrap();
        let count: u32 = String::from_utf8_lossy(&data).parse().unwrap();
        assert!(count > 0);
    }

    #[tokio::test]
    async fn test_read_dir_root() {
        let fs = ServerInfoFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(!entries.is_empty());
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/version")));
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/uptime")));
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/memory")));
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/cpu")));
    }

    #[tokio::test]
    async fn test_read_dir_memory() {
        let fs = ServerInfoFS::new();
        let entries = fs.read_dir(&PathBuf::from("/memory")).await.unwrap();
        assert!(!entries.is_empty());
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/memory/total")));
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/memory/used")));
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/memory/available")));
    }

    #[tokio::test]
    async fn test_read_plugins_list() {
        let fs = ServerInfoFS::new();
        let data = fs
            .read(&PathBuf::from("/plugins/list"), 0, -1)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&data),
            "hellofs,heartbeatfs,serverinfofs"
        );
    }

    #[tokio::test]
    async fn test_uptime_increases() {
        let fs = ServerInfoFS::new();
        let uptime1 = fs.read(&PathBuf::from("/uptime"), 0, -1).await.unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        fs.update();
        let uptime2 = fs.read(&PathBuf::from("/uptime"), 0, -1).await.unwrap();

        let u1: u64 = String::from_utf8_lossy(&uptime1).parse().unwrap();
        let u2: u64 = String::from_utf8_lossy(&uptime2).parse().unwrap();
        assert!(u2 >= u1);
    }

    #[tokio::test]
    async fn test_read_offset() {
        let fs = ServerInfoFS::new();
        let full_data = fs.read(&PathBuf::from("/version"), 0, -1).await.unwrap();
        let partial_data = fs.read(&PathBuf::from("/version"), 1, 2).await.unwrap();
        assert_eq!(partial_data, full_data[1..3].to_vec());
    }

    #[tokio::test]
    async fn test_write_fails() {
        let fs = ServerInfoFS::new();
        let result = fs
            .write(&PathBuf::from("/version"), b"test", 0, WriteFlags::none())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_fails() {
        let fs = ServerInfoFS::new();
        let result = fs.create(&PathBuf::from("/newfile")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stat_version() {
        let fs = ServerInfoFS::new();
        let meta = fs.stat(&PathBuf::from("/version")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/version"));
        assert!(meta.is_file());
    }

    #[tokio::test]
    async fn test_read_nonexistent_returns_not_found() {
        let fs = ServerInfoFS::new();
        let result = fs.read(&PathBuf::from("/nonexistent"), 0, -1).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
        let stat_result = fs.stat(&PathBuf::from("/nonexistent")).await;
        assert!(matches!(stat_result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_write_returns_permission_denied() {
        let fs = ServerInfoFS::new();
        let result = fs
            .write(&PathBuf::from("/version"), b"x", 0, WriteFlags::none())
            .await;
        assert!(matches!(
            result,
            Err(FSError::PermissionDenied { .. }) | Err(FSError::ReadOnly)
        ));
    }

    #[tokio::test]
    async fn test_create_returns_permission_denied() {
        let fs = ServerInfoFS::new();
        let result = fs.create(&PathBuf::from("/newfile")).await;
        assert!(matches!(
            result,
            Err(FSError::PermissionDenied { .. }) | Err(FSError::ReadOnly)
        ));
    }

    #[tokio::test]
    async fn test_mkdir_returns_permission_denied() {
        let fs = ServerInfoFS::new();
        let result = fs.mkdir(&PathBuf::from("/newdir"), 0o755).await;
        assert!(matches!(
            result,
            Err(FSError::PermissionDenied { .. }) | Err(FSError::ReadOnly)
        ));
    }

    #[tokio::test]
    async fn test_remove_returns_permission_denied() {
        let fs = ServerInfoFS::new();
        let result = fs.remove(&PathBuf::from("/version")).await;
        assert!(matches!(
            result,
            Err(FSError::PermissionDenied { .. }) | Err(FSError::ReadOnly)
        ));
    }
}
