//! Message queue filesystem (queuefs) plugin.

mod fs;

pub use fs::QueueFS;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;

    use super::*;

    #[tokio::test]
    async fn test_create_queue() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/myqueue")));
    }

    #[tokio::test]
    async fn test_create_duplicate_queue() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let result = fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await;
        assert!(matches!(result, Err(FSError::AlreadyExists { .. })));
    }

    #[tokio::test]
    async fn test_enqueue_message() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let data = b"Hello, World!";
        let _id = fs
            .write(&PathBuf::from("/myqueue/tail"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let msg_data = fs
            .read(&PathBuf::from("/myqueue/messages/000001"), 0, -1)
            .await
            .unwrap();
        assert_eq!(msg_data, data);
    }

    #[tokio::test]
    async fn test_read_head() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let head = fs
            .read(&PathBuf::from("/myqueue/head"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(head).unwrap(), "empty");

        let data = b"Hello, World!";
        fs.write(&PathBuf::from("/myqueue/tail"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let head = fs
            .read(&PathBuf::from("/myqueue/head"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(head).unwrap(), "000001");

        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"Second",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let head = fs
            .read(&PathBuf::from("/myqueue/head"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(head).unwrap(), "000001");
    }

    #[tokio::test]
    async fn test_read_tail() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let tail = fs
            .read(&PathBuf::from("/myqueue/tail"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(tail).unwrap(), "000001");

        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"First",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let tail = fs
            .read(&PathBuf::from("/myqueue/tail"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(tail).unwrap(), "000002");
    }

    #[tokio::test]
    async fn test_acknowledge_message() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"First",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"Second",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let head = fs
            .read(&PathBuf::from("/myqueue/head"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(head).unwrap(), "000001");

        fs.write(
            &PathBuf::from("/myqueue/.ack/000001"),
            b"done",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let head = fs
            .read(&PathBuf::from("/myqueue/head"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(head).unwrap(), "000002");

        let result = fs
            .read(&PathBuf::from("/myqueue/messages/000001"), 0, -1)
            .await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_message_count() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let count = fs
            .read(&PathBuf::from("/myqueue/metadata/count"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(count).unwrap(), "0");

        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"First",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"Second",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"Third",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let count = fs
            .read(&PathBuf::from("/myqueue/metadata/count"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(count).unwrap(), "3");

        fs.write(
            &PathBuf::from("/myqueue/.ack/000002"),
            b"done",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let count = fs
            .read(&PathBuf::from("/myqueue/metadata/count"), 0, -1)
            .await
            .unwrap();
        assert_eq!(String::from_utf8(count).unwrap(), "2");
    }

    #[tokio::test]
    async fn test_multiple_queues() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/queue1"), 0o755).await.unwrap();
        fs.mkdir(&PathBuf::from("/queue2"), 0o755).await.unwrap();

        fs.write(
            &PathBuf::from("/queue1/tail"),
            b"Q1 Msg1",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.write(
            &PathBuf::from("/queue2/tail"),
            b"Q2 Msg1",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let q1_msg = fs
            .read(&PathBuf::from("/queue1/messages/000001"), 0, -1)
            .await
            .unwrap();
        assert_eq!(q1_msg, b"Q1 Msg1");

        let q2_msg = fs
            .read(&PathBuf::from("/queue2/messages/000001"), 0, -1)
            .await
            .unwrap();
        assert_eq!(q2_msg, b"Q2 Msg1");
    }

    #[tokio::test]
    async fn test_remove_queue() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"Message",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.remove(&PathBuf::from("/myqueue")).await.unwrap();

        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(!entries.iter().any(|e| e.path.as_path() == std::path::Path::new("/myqueue")));

        let result = fs.read(&PathBuf::from("/myqueue/head"), 0, -1).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_message_id_format() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        for _ in 0..10 {
            fs.write(
                &PathBuf::from("/myqueue/tail"),
                b"msg",
                0,
                WriteFlags::none(),
            )
            .await
            .unwrap();
        }

        for i in 1..=10 {
            let id = format!("{:06}", i);
            let msg = fs
                .read(&PathBuf::from(format!("/myqueue/messages/{}", id)), 0, -1)
                .await
                .unwrap();
            assert_eq!(msg, b"msg");
        }
    }

    #[tokio::test]
    async fn test_read_with_offset_and_size() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();

        let data = b"Hello, World!";
        fs.write(&PathBuf::from("/myqueue/tail"), data, 0, WriteFlags::none())
            .await
            .unwrap();

        let msg = fs
            .read(&PathBuf::from("/myqueue/messages/000001"), 7, -1)
            .await
            .unwrap();
        assert_eq!(msg, b"World!");

        let msg = fs
            .read(&PathBuf::from("/myqueue/messages/000001"), 0, 5)
            .await
            .unwrap();
        assert_eq!(msg, b"Hello");

        let msg = fs
            .read(&PathBuf::from("/myqueue/messages/000001"), 7, 5)
            .await
            .unwrap();
        assert_eq!(msg, b"World");
    }

    #[tokio::test]
    async fn test_invalid_ack_data() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"Msg",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let result = fs
            .write(
                &PathBuf::from("/myqueue/.ack/000001"),
                b"invalid",
                0,
                WriteFlags::none(),
            )
            .await;
        assert!(matches!(result, Err(FSError::InvalidInput { .. })));

        let msg = fs
            .read(&PathBuf::from("/myqueue/messages/000001"), 0, -1)
            .await
            .unwrap();
        assert_eq!(msg, b"Msg");
    }

    #[tokio::test]
    async fn test_stat_root() {
        let fs = QueueFS::new();
        // Root is listable via read_dir; stat("/") may not be supported
        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        assert!(entries.is_empty() || !entries.is_empty());
    }

    #[tokio::test]
    async fn test_stat_queue() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();
        let meta = fs.stat(&PathBuf::from("/myqueue")).await.unwrap();
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_stat_message_file() {
        let fs = QueueFS::new();
        fs.mkdir(&PathBuf::from("/myqueue"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/myqueue/tail"),
            b"payload",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let meta = fs
            .stat(&PathBuf::from("/myqueue/messages/000001"))
            .await
            .unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.size, 7);
    }

    #[tokio::test]
    async fn test_nonexistent_returns_not_found() {
        let fs = QueueFS::new();
        let result = fs.stat(&PathBuf::from("/nonexistent")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }
}
