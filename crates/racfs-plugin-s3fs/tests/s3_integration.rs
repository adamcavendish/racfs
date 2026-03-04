//! Integration tests for S3FS against RustFS (S3-compatible).
//!
//! Requires Docker. Run with: `cargo test -p racfs-plugin-s3fs -- --ignored`

use std::path::PathBuf;
use std::time::Duration;

use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};
use racfs_core::flags::WriteFlags;
use racfs_plugin_s3fs::{S3Config, S3FS};
use testcontainers::core::IntoContainerPort;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};

const RUSTFS_ACCESS_KEY: &str = "rustfsadmin";
const RUSTFS_SECRET_KEY: &str = "rustfsadmin";
const TEST_BUCKET: &str = "testbucket";

/// Start a RustFS container and return (endpoint_url, container guard).
async fn start_rustfs() -> (String, testcontainers::core::ContainerAsync<GenericImage>) {
    let image = GenericImage::new("rustfs/rustfs", "latest")
        .with_exposed_port(9000.tcp())
        .with_wait_for(WaitFor::seconds(10))
        .with_env_var("RUSTFS_ACCESS_KEY", RUSTFS_ACCESS_KEY)
        .with_env_var("RUSTFS_SECRET_KEY", RUSTFS_SECRET_KEY)
        .with_cmd(["/tmp"]);

    let container = image
        .start()
        .await
        .expect("Failed to start RustFS container");
    let port = container
        .get_host_port_ipv4(9000)
        .await
        .expect("Failed to get port 9000");
    let endpoint = format!("http://127.0.0.1:{port}");
    (endpoint, container)
}

/// Create the test bucket via S3 API (retries until RustFS is ready).
async fn ensure_bucket(endpoint: &str, bucket: &str) {
    use aws_sdk_s3::error::SdkError;

    let creds = Credentials::new(
        RUSTFS_ACCESS_KEY,
        RUSTFS_SECRET_KEY,
        None,
        None,
        "racfs-s3fs-test",
    );
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .credentials_provider(creds)
        .load()
        .await;
    let client = aws_sdk_s3::Client::new(&config);

    for attempt in 1..=15 {
        match client.create_bucket().bucket(bucket).send().await {
            Ok(_) => return,
            Err(e) => {
                let already_exists = matches!(&e, SdkError::ServiceError(se)
                    if se.err().is_bucket_already_owned_by_you() || se.err().is_bucket_already_exists());
                if already_exists {
                    return;
                }
                if attempt == 15 {
                    panic!("CreateBucket failed after 15 attempts (endpoint={endpoint}): {e}");
                }
                tokio::time::sleep(Duration::from_millis(1000 * attempt)).await;
            }
        }
    }
}

fn s3_config(endpoint: &str) -> S3Config {
    S3Config {
        bucket: TEST_BUCKET.to_string(),
        region: "us-east-1".to_string(),
        endpoint: Some(endpoint.to_string()),
        access_key: RUSTFS_ACCESS_KEY.to_string(),
        secret_key: RUSTFS_SECRET_KEY.to_string(),
        cache_enabled: false,
        cache_size: 1024 * 1024 * 1024,
        multipart_threshold: 5 * 1024 * 1024,
        multipart_part_size: 5 * 1024 * 1024,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Docker; run with: cargo test -p racfs-plugin-s3fs -- --ignored"]
async fn test_rustfs_bucket_listing() {
    let (endpoint, _guard) = start_rustfs().await;
    ensure_bucket(&endpoint, TEST_BUCKET).await;
    let config = s3_config(&endpoint);
    let fs = S3FS::new(config).expect("S3FS::new");

    let result = DirFS::read_dir(&fs, &PathBuf::from("/")).await;
    assert!(result.is_ok(), "Failed to list bucket: {:?}", result.err());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Docker; run with: cargo test -p racfs-plugin-s3fs -- --ignored"]
async fn test_rustfs_stat_root() {
    let (endpoint, _guard) = start_rustfs().await;
    ensure_bucket(&endpoint, TEST_BUCKET).await;
    let config = s3_config(&endpoint);
    let fs = S3FS::new(config).expect("S3FS::new");

    let result = ReadFS::stat(&fs, &PathBuf::from("/")).await;
    assert!(result.is_ok(), "Stat root failed: {:?}", result.err());
    let metadata = result.unwrap();
    assert!(metadata.is_directory());
    assert_eq!(metadata.path, PathBuf::from("/"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Docker; run with: cargo test -p racfs-plugin-s3fs -- --ignored"]
async fn test_rustfs_file_write_read() {
    let (endpoint, _guard) = start_rustfs().await;
    ensure_bucket(&endpoint, TEST_BUCKET).await;
    let config = s3_config(&endpoint);
    let fs = S3FS::new(config).expect("S3FS::new");

    let test_path = "/test-racfs-file.txt";
    let test_content = b"Hello from RACFS S3FS test!";

    let write_result = WriteFS::write(
        &fs,
        &PathBuf::from(test_path),
        test_content,
        0,
        WriteFlags::none(),
    )
    .await;
    assert!(
        write_result.is_ok(),
        "Write failed: {:?}",
        write_result.err()
    );

    let read_result = ReadFS::read(&fs, &PathBuf::from(test_path), 0, -1).await;
    assert!(read_result.is_ok(), "Read failed: {:?}", read_result.err());
    assert_eq!(read_result.unwrap(), test_content);

    let _ = DirFS::remove(&fs, &PathBuf::from(test_path)).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Docker; run with: cargo test -p racfs-plugin-s3fs -- --ignored"]
async fn test_rustfs_file_delete() {
    let (endpoint, _guard) = start_rustfs().await;
    ensure_bucket(&endpoint, TEST_BUCKET).await;
    let config = s3_config(&endpoint);
    let fs = S3FS::new(config).expect("S3FS::new");

    let test_path = "/test-delete-file.txt";
    let test_content = b"Delete me please";

    let _ = WriteFS::create(&fs, &PathBuf::from(test_path)).await;
    let _ = WriteFS::write(
        &fs,
        &PathBuf::from(test_path),
        test_content,
        0,
        WriteFlags::none(),
    )
    .await;

    let delete_result = DirFS::remove(&fs, &PathBuf::from(test_path)).await;
    assert!(
        delete_result.is_ok(),
        "Delete failed: {:?}",
        delete_result.err()
    );

    let read_result = ReadFS::read(&fs, &PathBuf::from(test_path), 0, -1).await;
    assert!(read_result.is_err(), "File should not exist after deletion");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Docker; run with: cargo test -p racfs-plugin-s3fs -- --ignored"]
async fn test_rustfs_error_handling() {
    let (endpoint, _guard) = start_rustfs().await;
    ensure_bucket(&endpoint, TEST_BUCKET).await;
    let config = s3_config(&endpoint);
    let fs = S3FS::new(config).expect("S3FS::new");

    let result = ReadFS::read(&fs, &PathBuf::from("/non-existent-file-12345.txt"), 0, -1).await;
    assert!(result.is_err(), "Should return error for non-existent file");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "Requires Docker; run with: cargo test -p racfs-plugin-s3fs -- --ignored"]
async fn test_rustfs_file_stat() {
    let (endpoint, _guard) = start_rustfs().await;
    ensure_bucket(&endpoint, TEST_BUCKET).await;
    let config = s3_config(&endpoint);
    let fs = S3FS::new(config).expect("S3FS::new");

    let test_path = "/test-stat-file.txt";
    let test_content = b"Stat test content";

    let _ = WriteFS::create(&fs, &PathBuf::from(test_path)).await;
    let _ = WriteFS::write(
        &fs,
        &PathBuf::from(test_path),
        test_content,
        0,
        WriteFlags::none(),
    )
    .await;

    let stat_result = ReadFS::stat(&fs, &PathBuf::from(test_path)).await;
    assert!(stat_result.is_ok(), "Stat failed: {:?}", stat_result.err());
    let metadata = stat_result.unwrap();
    assert!(metadata.size > 0, "File should have content");

    let _ = DirFS::remove(&fs, &PathBuf::from(test_path)).await;
}
