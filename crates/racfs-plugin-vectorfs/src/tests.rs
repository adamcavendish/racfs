#[cfg(test)]
mod vectorfs_tests {
    use crate::{VectorConfig, VectorFS};
    use racfs_core::error::FSError;
    use racfs_core::filesystem::{DirFS, FileSystem, ReadFS, WriteFS};
    use racfs_core::flags::WriteFlags;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_vector_config_default() {
        let config = VectorConfig::default();
        assert_eq!(config.table_name, "vectors");
        assert!(config.embedding_api.is_none());
        assert_eq!(config.dimension, 384);
        assert!(!config.storage_uri.is_empty());
    }

    #[tokio::test]
    async fn test_vectorfs_new_async() {
        let dir = tempfile::tempdir().unwrap();
        let config = VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        };
        let fs = VectorFS::with_config_async(config).await.unwrap();
        assert_eq!(fs.config.table_name, "vectors");
        assert_eq!(fs.config.dimension, 384);
    }

    #[tokio::test]
    async fn test_vectorfs_with_config_async() {
        let dir = tempfile::tempdir().unwrap();
        let storage_uri = dir.path().to_string_lossy().to_string();
        let config = VectorConfig {
            storage_uri,
            embedding_api: Some("http://localhost:8080".to_string()),
            table_name: "my_vectors".to_string(),
            dimension: 768,
        };
        let fs = VectorFS::with_config_async(config.clone()).await.unwrap();
        assert_eq!(fs.config.dimension, 768);
    }

    /// With embedding_api set, writing non-UTF-8 document content returns InvalidInput.
    #[tokio::test]
    async fn test_embedding_api_requires_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let config = VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            embedding_api: Some("http://localhost:9999".to_string()),
            table_name: "vectors".to_string(),
            dimension: 384,
        };
        let fs = VectorFS::with_config_async(config).await.unwrap();
        fs.create(&PathBuf::from("/documents/doc.txt"))
            .await
            .unwrap();
        let non_utf8 = [0xffu8, 0xfe];
        let err = fs
            .write(
                &PathBuf::from("/documents/doc.txt"),
                &non_utf8,
                0,
                WriteFlags::none(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, FSError::InvalidInput { .. }));
        let msg = err.to_string();
        assert!(msg.contains("UTF-8"));
    }

    /// Pre-computed vector via xattr "racfs.vector": write uses it instead of embedding API.
    #[tokio::test]
    async fn test_precomputed_vector_via_xattr() {
        let dir = tempfile::tempdir().unwrap();
        let config = VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            embedding_api: None,
            table_name: "vectors".to_string(),
            dimension: 3,
        };
        let fs = VectorFS::with_config_async(config).await.unwrap();
        let path = PathBuf::from("/documents/precomputed.txt");

        fs.create(&path).await.unwrap();
        let vector_json = b"[1.0, 0.0, 0.0]";
        fs.set_xattr(&path, "racfs.vector", vector_json)
            .await
            .unwrap();
        fs.write(&path, b"content", 0, WriteFlags::none())
            .await
            .unwrap();

        let names = fs.list_xattr(&path).await.unwrap();
        assert!(names.contains(&"racfs.vector".to_string()));
        let value = fs.get_xattr(&path, "racfs.vector").await.unwrap();
        assert_eq!(value, vector_json);

        fs.mkdir(&PathBuf::from("/search/q1"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/search/q1/query.txt"),
            b"test",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let matches_data = fs
            .read(&PathBuf::from("/search/q1/matches.txt"), 0, -1)
            .await
            .unwrap();
        let matches: Vec<serde_json::Value> = serde_json::from_slice(&matches_data).unwrap();
        assert!(matches.len() <= 10);
    }

    /// Persist to storage_uri: document written in one instance is visible in a new instance.
    #[tokio::test]
    async fn test_persist_to_storage_uri() {
        let dir = tempfile::tempdir().unwrap();
        let storage_uri = dir.path().to_string_lossy().to_string();
        let config = VectorConfig {
            storage_uri: storage_uri.clone(),
            embedding_api: None,
            table_name: "vectors".to_string(),
            dimension: 3,
        };

        {
            let fs = VectorFS::with_config_async(config.clone()).await.unwrap();
            fs.create(&PathBuf::from("/documents/persisted.txt"))
                .await
                .unwrap();
            fs.write(
                &PathBuf::from("/documents/persisted.txt"),
                b"hello from first instance",
                0,
                WriteFlags::none(),
            )
            .await
            .unwrap();
        }

        let fs2 = VectorFS::with_config_async(config).await.unwrap();
        let data = fs2
            .read(&PathBuf::from("/documents/persisted.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(data, b"hello from first instance");
        let entries = fs2.read_dir(&PathBuf::from("/documents")).await.unwrap();
        assert!(entries
            .iter()
            .any(|e| e.path.as_path() == std::path::Path::new("/documents/persisted.txt")));
    }

    #[tokio::test]
    async fn test_index_dimension() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let dim_data = fs
            .read(&PathBuf::from("/index/dimension"), 0, -1)
            .await
            .unwrap();
        let dim_str = String::from_utf8(dim_data).unwrap();
        assert_eq!(dim_str, "384");
    }

    #[tokio::test]
    async fn test_search_empty() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let query = vec![1.0f32; 384];
        let results = fs.search(&query, 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_create_and_read_document() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        fs.create(&PathBuf::from("/documents/doc1.txt"))
            .await
            .unwrap();

        let data = b"Hello, VectorFS!";
        fs.write(
            &PathBuf::from("/documents/doc1.txt"),
            data,
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let read = fs
            .read(&PathBuf::from("/documents/doc1.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(read, data);
    }

    #[tokio::test]
    async fn test_document_count() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        assert_eq!(fs.document_count().await.unwrap(), 0);

        fs.create(&PathBuf::from("/documents/doc1.txt"))
            .await
            .unwrap();
        fs.write(
            &PathBuf::from("/documents/doc1.txt"),
            b"Test document",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        assert_eq!(fs.document_count().await.unwrap(), 1);

        let count_data = fs
            .read(&PathBuf::from("/index/count"), 0, -1)
            .await
            .unwrap();
        let count_str = String::from_utf8(count_data).unwrap();
        assert_eq!(count_str, "1");
    }

    #[tokio::test]
    async fn test_index_status() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let status_data = fs
            .read(&PathBuf::from("/index/status"), 0, -1)
            .await
            .unwrap();
        let status_str = String::from_utf8(status_data).unwrap();
        assert_eq!(status_str, "ready");
    }

    #[tokio::test]
    async fn test_read_dir_root() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/")).await.unwrap();
        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();

        assert!(paths.contains(&PathBuf::from("/documents")));
        assert!(paths.contains(&PathBuf::from("/index")));
        assert!(paths.contains(&PathBuf::from("/search")));
    }

    #[tokio::test]
    async fn test_read_dir_documents() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        fs.create(&PathBuf::from("/documents/doc1.txt"))
            .await
            .unwrap();
        fs.create(&PathBuf::from("/documents/doc2.txt"))
            .await
            .unwrap();

        let entries = fs.read_dir(&PathBuf::from("/documents")).await.unwrap();
        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();

        assert!(paths.contains(&PathBuf::from("/documents/doc1.txt")));
        assert!(paths.contains(&PathBuf::from("/documents/doc2.txt")));
    }

    #[tokio::test]
    async fn test_remove_document() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        fs.create(&PathBuf::from("/documents/doc1.txt"))
            .await
            .unwrap();
        fs.write(
            &PathBuf::from("/documents/doc1.txt"),
            b"Test",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        assert_eq!(fs.document_count().await.unwrap(), 1);

        fs.remove(&PathBuf::from("/documents/doc1.txt"))
            .await
            .unwrap();

        assert_eq!(fs.document_count().await.unwrap(), 0);
        assert!(
            fs.stat(&PathBuf::from("/documents/doc1.txt"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_write_virtual_path_fails() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let result = fs
            .write(
                &PathBuf::from("/index/count"),
                b"100",
                0,
                WriteFlags::none(),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_outside_documents_fails() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let result = fs.create(&PathBuf::from("/somefile.txt")).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rename_document() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        fs.create(&PathBuf::from("/documents/old.txt"))
            .await
            .unwrap();
        fs.write(
            &PathBuf::from("/documents/old.txt"),
            b"content",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.rename(
            &PathBuf::from("/documents/old.txt"),
            &PathBuf::from("/documents/new.txt"),
        )
        .await
        .unwrap();

        assert!(fs.stat(&PathBuf::from("/documents/old.txt")).await.is_err());
        let meta = fs.stat(&PathBuf::from("/documents/new.txt")).await.unwrap();
        assert_eq!(meta.size, 7);
        let data = fs
            .read(&PathBuf::from("/documents/new.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(data, b"content");
    }

    #[tokio::test]
    async fn test_rename_document_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        fs.create(&PathBuf::from("/documents/src.txt"))
            .await
            .unwrap();
        fs.write(
            &PathBuf::from("/documents/src.txt"),
            b"source",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        fs.create(&PathBuf::from("/documents/dst.txt"))
            .await
            .unwrap();
        fs.write(
            &PathBuf::from("/documents/dst.txt"),
            b"old",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.rename(
            &PathBuf::from("/documents/src.txt"),
            &PathBuf::from("/documents/dst.txt"),
        )
        .await
        .unwrap();

        assert!(fs.stat(&PathBuf::from("/documents/src.txt")).await.is_err());
        let data = fs
            .read(&PathBuf::from("/documents/dst.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(data, b"source");
    }

    #[tokio::test]
    async fn test_rename_index_forbidden() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let result = fs
            .rename(
                &PathBuf::from("/index/count"),
                &PathBuf::from("/index/other"),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_query_and_matches() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        fs.create(&PathBuf::from("/documents/doc1.txt"))
            .await
            .unwrap();
        fs.write(
            &PathBuf::from("/documents/doc1.txt"),
            b"hello world",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        fs.mkdir(&PathBuf::from("/search/q1"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/search/q1/query.txt"),
            b"hello",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let matches_data = fs
            .read(&PathBuf::from("/search/q1/matches.txt"), 0, -1)
            .await
            .unwrap();
        let matches: Vec<serde_json::Value> = serde_json::from_slice(&matches_data).unwrap();
        assert!(matches.len() <= 10);

        // Query with JSON {"query": "...", "limit": N}: limit is honored
        fs.mkdir(&PathBuf::from("/search/q2"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/search/q2/query.txt"),
            b"{\"query\": \"hello\", \"limit\": 2}",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let matches_data2 = fs
            .read(&PathBuf::from("/search/q2/matches.txt"), 0, -1)
            .await
            .unwrap();
        let matches2: Vec<serde_json::Value> = serde_json::from_slice(&matches_data2).unwrap();
        assert!(matches2.len() <= 2);

        let query_data = fs
            .read(&PathBuf::from("/search/q1/query.txt"), 0, -1)
            .await
            .unwrap();
        assert_eq!(query_data, b"hello");

        let entries = fs.read_dir(&PathBuf::from("/search/q1")).await.unwrap();
        let names: Vec<_> = entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"query.txt".to_string()));
        assert!(names.contains(&"matches.txt".to_string()));
    }

    /// query.txt can be JSON {"query": "...", "limit": N}; matches.txt returns at most N results.
    #[tokio::test]
    async fn test_query_txt_json_with_limit() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        for i in 0..5 {
            let path = PathBuf::from(format!("/documents/doc{}.txt", i));
            fs.create(&path).await.unwrap();
            fs.write(&path, b"content", 0, WriteFlags::none())
                .await
                .unwrap();
        }

        fs.mkdir(&PathBuf::from("/search/q2"), 0o755).await.unwrap();
        fs.write(
            &PathBuf::from("/search/q2/query.txt"),
            br#"{"query": "content", "limit": 2}"#,
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();

        let matches_data = fs
            .read(&PathBuf::from("/search/q2/matches.txt"), 0, -1)
            .await
            .unwrap();
        let matches: Vec<serde_json::Value> = serde_json::from_slice(&matches_data).unwrap();
        assert_eq!(matches.len(), 2, "limit=2 should return at most 2 results");
    }

    #[tokio::test]
    async fn test_stat_root() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let meta = fs.stat(&PathBuf::from("/")).await.unwrap();
        assert!(meta.is_directory());
    }

    #[tokio::test]
    async fn test_stat_file() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        fs.create(&PathBuf::from("/documents/f.txt")).await.unwrap();
        fs.write(
            &PathBuf::from("/documents/f.txt"),
            b"content",
            0,
            WriteFlags::none(),
        )
        .await
        .unwrap();
        let meta = fs.stat(&PathBuf::from("/documents/f.txt")).await.unwrap();
        assert_eq!(meta.path, PathBuf::from("/documents/f.txt"));
        assert_eq!(meta.size, 7);
    }

    #[tokio::test]
    async fn test_nonexistent_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let fs = VectorFS::with_config_async(VectorConfig {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let result = fs.stat(&PathBuf::from("/documents/nonexistent.txt")).await;
        assert!(matches!(result, Err(FSError::NotFound { .. })));
    }
}
