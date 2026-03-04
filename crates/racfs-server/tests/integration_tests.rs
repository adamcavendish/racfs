//! Integration tests for RACFS server API.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::{delete, get, post, put};
use racfs_plugin_devfs::DevFS;
use racfs_plugin_memfs::MemFS;
use racfs_vfs::MountableFS;
use reqwest::StatusCode;

use racfs_server::api::{directories, files, health, metadata};
use racfs_server::auth::JwtConfig;
use racfs_server::state::AppState;

/// Start a test server and return its base URL.
async fn start_test_server() -> (String, tokio::task::JoinHandle<()>) {
    // Create the virtual filesystem
    let vfs = Arc::new(MountableFS::new());

    // Mount test filesystems
    let memfs = Arc::new(MemFS::new()) as Arc<dyn racfs_core::FileSystem>;
    let devfs = Arc::new(DevFS::new()) as Arc<dyn racfs_core::FileSystem>;

    vfs.mount(PathBuf::from("/memfs"), memfs)
        .expect("Failed to mount memfs");
    vfs.mount(PathBuf::from("/dev"), devfs)
        .expect("Failed to mount devfs");

    // Create application state
    let state =
        AppState::new(vfs, JwtConfig::default(), 2, vec![]).expect("Failed to create AppState");

    // Build the router
    let app = Router::new()
        .route("/api/v1/health", get(health::health_check))
        .route("/api/v1/files", get(files::read_file))
        .route("/api/v1/files", post(files::create_file))
        .route("/api/v1/files", put(files::write_file))
        .route("/api/v1/files", delete(files::delete_file))
        .route("/api/v1/directories", get(directories::list_directory))
        .route("/api/v1/directories", post(directories::create_directory))
        .route("/api/v1/stat", get(metadata::stat))
        .route("/api/v1/rename", post(metadata::rename))
        .route("/api/v1/chmod", post(metadata::chmod))
        .with_state(state);

    // Bind to a random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to address");
    let addr = listener.local_addr().expect("Failed to get local address");

    let base_url = format!("http://{}", addr);

    // Spawn the server
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    // Give the server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    (base_url, handle)
}

#[tokio::test]
async fn test_health_endpoint() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/v1/health", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/files?path=/memfs/test.txt", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("Created file"));
    assert!(body.contains("test.txt"));
}

#[tokio::test]
async fn test_write_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create file first
    client
        .post(format!(
            "{}/api/v1/files?path=/memfs/write_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to create file");

    // Write to file
    let response = client
        .put(format!("{}/api/v1/files", base_url))
        .json(&serde_json::json!({
            "path": "/memfs/write_test.txt",
            "data": "Hello, World!",
            "offset": 0
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("Wrote 13 bytes"));
}

#[tokio::test]
async fn test_read_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create and write to file
    client
        .post(format!(
            "{}/api/v1/files?path=/memfs/read_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to create file");

    client
        .put(format!("{}/api/v1/files", base_url))
        .json(&serde_json::json!({
            "path": "/memfs/read_test.txt",
            "data": "Test content",
            "offset": 0
        }))
        .send()
        .await
        .expect("Failed to write file");

    // Read file - response is JSON with the content as a string
    let response = client
        .get(format!(
            "{}/api/v1/files?path=/memfs/read_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let content: String = response
        .json()
        .await
        .expect("Failed to parse JSON response");
    assert_eq!(content, "Test content");
}

#[tokio::test]
async fn test_delete_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create file first
    client
        .post(format!(
            "{}/api/v1/files?path=/memfs/delete_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to create file");

    // Delete file
    let response = client
        .delete(format!(
            "{}/api/v1/files?path=/memfs/delete_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("Deleted"));
}

#[tokio::test]
async fn test_create_directory() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/directories", base_url))
        .json(&serde_json::json!({
            "path": "/memfs/test_dir",
            "perm": 0o755
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("Created directory"));
    assert!(body.contains("test_dir"));
}

#[tokio::test]
async fn test_list_directory() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create some files and directories
    client
        .post(format!("{}/api/v1/files?path=/memfs/file1.txt", base_url))
        .send()
        .await
        .expect("Failed to create file");
    client
        .post(format!("{}/api/v1/directories", base_url))
        .json(&serde_json::json!({
            "path": "/memfs/dir1",
        }))
        .send()
        .await
        .expect("Failed to create directory");

    // List directory
    let response = client
        .get(format!("{}/api/v1/directories?path=/memfs", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");

    // Verify entries are present
    assert!(body.contains("file1.txt") || body.contains("dir1"));
}

#[tokio::test]
async fn test_stat_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create a file
    client
        .post(format!(
            "{}/api/v1/files?path=/memfs/stat_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to create file");

    // Stat the file
    let response = client
        .get(format!(
            "{}/api/v1/stat?path=/memfs/stat_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("file") || body.contains("File"));
    assert!(body.contains("stat_test.txt"));
}

#[tokio::test]
async fn test_rename_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create a file
    client
        .post(format!(
            "{}/api/v1/files?path=/memfs/old_name.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to create file");

    // Rename the file
    let response = client
        .post(format!("{}/api/v1/rename", base_url))
        .json(&serde_json::json!({
            "old_path": "/memfs/old_name.txt",
            "new_path": "/memfs/new_name.txt"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("Renamed"));
    assert!(body.contains("old_name.txt"));
    assert!(body.contains("new_name.txt"));
}

#[tokio::test]
async fn test_chmod_file() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    // Create a file
    client
        .post(format!(
            "{}/api/v1/files?path=/memfs/chmod_test.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to create file");

    // Change permissions
    let response = client
        .post(format!("{}/api/v1/chmod", base_url))
        .json(&serde_json::json!({
            "path": "/memfs/chmod_test.txt",
            "mode": 0o644
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("Changed permissions"));
}

#[tokio::test]
async fn test_create_file_already_exists_returns_409() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    let path = "/memfs/conflict.txt";
    let create1 = client
        .post(format!("{}/api/v1/files?path={}", base_url, path))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(create1.status(), StatusCode::OK);

    let create2 = client
        .post(format!("{}/api/v1/files?path={}", base_url, path))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(create2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_read_file_invalid_path_returns_400() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/v1/files?path=relative/path", base_url))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_file_nonexistent_returns_404() {
    let (base_url, _handle) = start_test_server().await;
    let client = reqwest::Client::new();

    let response = client
        .delete(format!(
            "{}/api/v1/files?path=/memfs/does_not_exist.txt",
            base_url
        ))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
