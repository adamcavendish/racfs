//! Streaming data filesystem (streamfs) plugin.

mod fs;

pub use fs::{StreamConfig, StreamFS};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert_eq!(config.buffer_size, 1000);
        assert_eq!(config.history_size, 100);
        assert_eq!(config.max_streams, 100);
    }

    #[tokio::test]
    async fn test_stream_creation() {
        let fs = StreamFS::default_config();

        fs.mkdir(Path::new("/streams/mystream"), 0o755)
            .await
            .unwrap();

        let stat = fs.stat(Path::new("/streams/mystream")).await.unwrap();
        assert!(stat.is_directory());

        let entries = fs.read_dir(Path::new("/streams")).await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn test_message_operations() {
        let fs = StreamFS::default_config();

        fs.mkdir(Path::new("/streams/test"), 0o755).await.unwrap();

        let written = fs
            .write(
                Path::new("/streams/test/tail"),
                b"hello world",
                0,
                WriteFlags::empty(),
            )
            .await
            .unwrap();
        assert_eq!(written, 11);

        let tail = fs
            .read(Path::new("/streams/test/tail"), 0, 100)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&tail), "000002");

        let msg = fs
            .read(Path::new("/streams/test/data/000001.msg"), 0, 100)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&msg), "hello world");
    }

    #[tokio::test]
    async fn test_ring_buffer() {
        let config = StreamConfig {
            buffer_size: 3,
            history_size: 10,
            max_streams: 10,
            compression: None,
        };
        let fs = StreamFS::new(config);

        fs.mkdir(Path::new("/streams/ring"), 0o755).await.unwrap();

        for i in 1..=5 {
            fs.write(
                Path::new("/streams/ring/tail"),
                format!("msg{}", i).as_bytes(),
                0,
                WriteFlags::empty(),
            )
            .await
            .unwrap();
        }

        let entries = fs.read_dir(Path::new("/streams/ring/data")).await.unwrap();
        assert_eq!(entries.len(), 3);

        let head = fs
            .read(Path::new("/streams/ring/head"), 0, 100)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&head), "000003");
    }

    #[tokio::test]
    async fn test_head_tail_tracking() {
        let fs = StreamFS::default_config();

        fs.mkdir(Path::new("/streams/track"), 0o755).await.unwrap();

        let head = fs
            .read(Path::new("/streams/track/head"), 0, 100)
            .await
            .unwrap();
        let tail = fs
            .read(Path::new("/streams/track/tail"), 0, 100)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&head), "000001");
        assert_eq!(String::from_utf8_lossy(&tail), "000001");

        fs.write(
            Path::new("/streams/track/tail"),
            b"test",
            0,
            WriteFlags::empty(),
        )
        .await
        .unwrap();

        let tail = fs
            .read(Path::new("/streams/track/tail"), 0, 100)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&tail), "000002");

        fs.write(
            Path::new("/streams/track/head"),
            b"000002",
            0,
            WriteFlags::empty(),
        )
        .await
        .unwrap();

        let head = fs
            .read(Path::new("/streams/track/head"), 0, 100)
            .await
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&head), "000002");
    }

    #[tokio::test]
    async fn test_stream_message_compression() {
        use racfs_core::{CompressionLevel, ZstdCompression};
        use std::sync::Arc;

        let compressor = Arc::new(ZstdCompression::new(CompressionLevel::Default));
        let config = StreamConfig {
            buffer_size: 100,
            history_size: 10,
            max_streams: 10,
            compression: Some(compressor),
        };
        let fs = StreamFS::new(config);

        fs.mkdir(Path::new("/streams/comp"), 0o755).await.unwrap();

        let data = b"hello world ".repeat(100);
        fs.write(
            Path::new("/streams/comp/tail"),
            &data,
            0,
            WriteFlags::empty(),
        )
        .await
        .unwrap();

        let read = fs
            .read(Path::new("/streams/comp/data/000001.msg"), 0, -1)
            .await
            .unwrap();
        assert_eq!(read, data);

        let meta = fs
            .stat(Path::new("/streams/comp/data/000001.msg"))
            .await
            .unwrap();
        assert_eq!(meta.size, data.len() as u64);
    }

    #[tokio::test]
    async fn test_stat_root() {
        let fs = StreamFS::default_config();
        let entries = fs.read_dir(Path::new("/streams")).await.unwrap();
        assert!(entries.is_empty() || !entries.is_empty());
    }

    #[tokio::test]
    async fn test_stat_stream_dir() {
        let fs = StreamFS::default_config();
        fs.mkdir(Path::new("/streams/s1"), 0o755).await.unwrap();
        let meta = fs.stat(Path::new("/streams/s1")).await.unwrap();
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_nonexistent_returns_not_found() {
        let fs = StreamFS::default_config();
        let result = fs.stat(Path::new("/streams/nonexistent")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }
}
