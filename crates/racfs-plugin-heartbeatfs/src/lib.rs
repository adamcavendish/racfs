//! Health monitoring filesystem (heartbeatfs) plugin.

mod fs;

pub use fs::HeartbeatFS;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::{error::FSError, flags::WriteFlags};

    use super::*;

    #[tokio::test]
    async fn test_create_new() {
        let fs = HeartbeatFS::new();
        assert_eq!(fs.inner.read().beats, 0);
    }

    #[tokio::test]
    async fn test_read_status() {
        let fs = HeartbeatFS::new();
        let status = fs.read(&PathBuf::from("/status"), 0, -1).await.unwrap();
        assert_eq!(status, b"ok");
    }

    #[tokio::test]
    async fn test_read_uptime() {
        let fs = HeartbeatFS::new();
        let uptime = fs.read(&PathBuf::from("/uptime"), 0, -1).await.unwrap();
        let uptime_str = String::from_utf8(uptime).unwrap();
        let _uptime_val: u64 = uptime_str.parse().unwrap();
    }

    #[tokio::test]
    async fn test_read_beats() {
        let fs = HeartbeatFS::new();
        let beats = fs.read(&PathBuf::from("/beats"), 0, -1).await.unwrap();
        assert_eq!(beats, b"0");
    }

    #[tokio::test]
    async fn test_read_last_beat() {
        let fs = HeartbeatFS::new();
        let last_beat = fs.read(&PathBuf::from("/last_beat"), 0, -1).await.unwrap();
        let last_beat_str = String::from_utf8(last_beat).unwrap();
        chrono::DateTime::parse_from_rfc3339(&last_beat_str).unwrap();
    }

    #[tokio::test]
    async fn test_write_pulse() {
        let fs = HeartbeatFS::new();
        fs.write(&PathBuf::from("/pulse"), b"beat", 0, WriteFlags::none())
            .await
            .unwrap();

        let beats = fs.read(&PathBuf::from("/beats"), 0, -1).await.unwrap();
        assert_eq!(beats, b"1");
    }

    #[tokio::test]
    async fn test_write_pulse_multiple() {
        let fs = HeartbeatFS::new();

        for _ in 0..5 {
            fs.write(&PathBuf::from("/pulse"), b"beat", 0, WriteFlags::none())
                .await
                .unwrap();
        }

        let beats = fs.read(&PathBuf::from("/beats"), 0, -1).await.unwrap();
        assert_eq!(beats, b"5");
    }

    #[tokio::test]
    async fn test_write_pulse_with_whitespace() {
        let fs = HeartbeatFS::new();
        fs.write(&PathBuf::from("/pulse"), b" beat ", 0, WriteFlags::none())
            .await
            .unwrap();

        let beats = fs.read(&PathBuf::from("/beats"), 0, -1).await.unwrap();
        assert_eq!(beats, b"1");
    }

    #[tokio::test]
    async fn test_write_pulse_invalid_value() {
        let fs = HeartbeatFS::new();
        let result = fs
            .write(&PathBuf::from("/pulse"), b"invalid", 0, WriteFlags::none())
            .await;
        assert!(matches!(result, Err(FSError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_write_readonly_status() {
        let fs = HeartbeatFS::new();
        let result = fs
            .write(&PathBuf::from("/status"), b"error", 0, WriteFlags::none())
            .await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_write_readonly_beats() {
        let fs = HeartbeatFS::new();
        let result = fs
            .write(&PathBuf::from("/beats"), b"999", 0, WriteFlags::none())
            .await;
        assert!(matches!(result, Err(FSError::ReadOnly)));
    }

    #[tokio::test]
    async fn test_read_dir() {
        let fs = HeartbeatFS::new();
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert_eq!(entries.len(), 5);

        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/status")));
        assert!(paths.contains(&PathBuf::from("/uptime")));
        assert!(paths.contains(&PathBuf::from("/beats")));
        assert!(paths.contains(&PathBuf::from("/last_beat")));
        assert!(paths.contains(&PathBuf::from("/pulse")));
    }

    #[tokio::test]
    async fn test_stat_root() {
        let fs = HeartbeatFS::new();
        let meta = fs.stat(&PathBuf::from("/")).await.unwrap();
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_stat() {
        let fs = HeartbeatFS::new();
        let metadata = fs.stat(&PathBuf::from("/status")).await.unwrap();
        assert_eq!(metadata.path, PathBuf::from("/status"));
        assert_eq!(metadata.size, 2);
    }

    #[tokio::test]
    async fn test_read_invalid_path() {
        let fs = HeartbeatFS::new();
        let result = fs.read(&PathBuf::from("/invalid"), 0, -1).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_stat_invalid_path() {
        let fs = HeartbeatFS::new();
        let result = fs.stat(&PathBuf::from("/invalid")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_read_dir_invalid_path() {
        let fs = HeartbeatFS::new();
        let result = fs.read_dir(&PathBuf::from("/status")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let fs = HeartbeatFS::new();
        let data = fs.read(&PathBuf::from("/status"), 1, 2).await.unwrap();
        assert_eq!(data, b"k");
    }

    #[tokio::test]
    async fn test_read_with_size() {
        let fs = HeartbeatFS::new();
        let data = fs.read(&PathBuf::from("/status"), 0, 1).await.unwrap();
        assert_eq!(data, b"o");
    }

    #[tokio::test]
    async fn test_default() {
        let fs = HeartbeatFS::default();
        let beats = fs.read(&PathBuf::from("/beats"), 0, -1).await.unwrap();
        assert_eq!(beats, b"0");
    }
}
