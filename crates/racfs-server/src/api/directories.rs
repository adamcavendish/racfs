//! Directory operation endpoints

use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    response::Json,
};
use racfs_core::metadata::FileMetadata;
use racfs_http_error::HttpErrorResponse;
use serde::Deserialize;
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::error::map_fs_error_with_context;
use crate::state::AppState;
use crate::validation;
use racfs_core::filesystem::DirFS;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct DirectoryQuery {
    pub path: String,
}

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct CreateDirectoryRequest {
    pub path: String,
    pub perm: Option<u32>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct DirectoryResponse {
    pub message: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct DirectoryListResponse {
    pub entries: Vec<DirectoryEntry>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct DirectoryEntry {
    pub permissions: String,
    pub size: u64,
    pub modified: Option<String>,
    pub path: String,
    #[serde(rename = "file_type")]
    pub file_type: String,
}

impl From<FileMetadata> for DirectoryEntry {
    fn from(metadata: FileMetadata) -> Self {
        DirectoryEntry {
            permissions: format!("{:o}", metadata.permissions()),
            size: metadata.size,
            modified: metadata
                .modified
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string()),
            path: metadata.path.to_string_lossy().to_string(),
            file_type: metadata.file_type().to_string(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/directories",
    tag = "directories",
    params(DirectoryQuery),
    responses(
        (status = 200, description = "Directory listing", body = DirectoryListResponse),
        (status = 404, description = "Directory not found"),
    ),
)]
#[instrument(skip(state))]
pub async fn list_directory(
    State(state): State<AppState>,
    Query(query): Query<DirectoryQuery>,
) -> Result<Json<DirectoryListResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, "Listing directory");

    let entries = state
        .vfs
        .read_dir(&path)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("read_dir"), Some(path.as_path())))?;

    state.vfs_metrics.directory_lists_total.inc();

    let dir_entries: Vec<DirectoryEntry> = entries.into_iter().map(DirectoryEntry::from).collect();

    Ok(Json(DirectoryListResponse {
        entries: dir_entries,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/directories",
    tag = "directories",
    request_body = CreateDirectoryRequest,
    responses(
        (status = 200, description = "Directory created", body = DirectoryResponse),
        (status = 409, description = "Directory already exists"),
    ),
)]
#[instrument(skip(state))]
pub async fn create_directory(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateDirectoryRequest>,
) -> Result<Json<DirectoryResponse>, HttpErrorResponse> {
    validation::validate_path(&body.path)?;
    let path = PathBuf::from(&body.path);
    let perm = body.perm.unwrap_or(0o755);
    info!(path = %body.path, perm = perm, "Creating directory");

    state
        .vfs
        .mkdir(&path, perm)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("mkdir"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);
    state.vfs_metrics.directory_creates_total.inc();

    Ok(Json(DirectoryResponse {
        message: format!("Created directory: {}", body.path),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::JwtConfig;
    use axum::extract::{Query, State};
    use racfs_plugin_memfs::MemFS;
    use racfs_vfs::MountableFS;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn test_app_state() -> AppState {
        let vfs = Arc::new(MountableFS::new());
        let memfs = Arc::new(MemFS::new()) as Arc<dyn racfs_core::FileSystem>;
        vfs.mount(PathBuf::from("/memfs"), memfs)
            .expect("mount memfs");
        AppState::new(vfs, JwtConfig::default(), 2, vec![]).expect("AppState")
    }

    #[tokio::test]
    async fn list_directory_root_memfs() {
        let state = test_app_state();
        let query = DirectoryQuery {
            path: "/memfs".to_string(),
        };
        let result = list_directory(State(state), Query(query)).await;
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.0.entries.is_empty() || json.0.entries.iter().any(|e| !e.path.is_empty()));
    }

    #[tokio::test]
    async fn list_directory_rejects_invalid_path() {
        let state = test_app_state();
        let query = DirectoryQuery {
            path: "not/absolute".to_string(),
        };
        let result = list_directory(State(state), Query(query)).await;
        let err = match &result {
            Ok(_) => panic!("expected validation error"),
            Err(e) => e,
        };
        assert_eq!(
            axum::http::StatusCode::from(err.code.clone()),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn create_directory_then_list() {
        let state = test_app_state();
        let path = "/memfs/unit_dir";

        let body = CreateDirectoryRequest {
            path: path.to_string(),
            perm: Some(0o755),
        };
        let create = create_directory(State(state.clone()), axum::Json(body)).await;
        assert!(create.is_ok());
        let msg = create.unwrap().0.message;
        assert!(msg.contains("Created directory") && msg.contains("unit_dir"));

        let list_query = DirectoryQuery {
            path: "/memfs".to_string(),
        };
        let list = list_directory(State(state), Query(list_query)).await;
        assert!(list.is_ok());
        let entries = list.unwrap().0.entries;
        assert!(entries.iter().any(|e| e.path.contains("unit_dir")));
    }
}
