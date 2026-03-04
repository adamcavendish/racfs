//! Metadata operation endpoints

use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    response::Json,
};
use racfs_http_error::HttpErrorResponse;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::error::map_fs_error_with_context;
use crate::state::AppState;
use crate::validation;
use racfs_core::filesystem::{ChmodFS, DirFS, FileSystem, ReadFS};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct StatQuery {
    pub path: String,
}

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct RenameRequest {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct ChmodRequest {
    pub path: String,
    pub mode: u32,
}

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct TruncateRequest {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct SymlinkRequest {
    pub target: String,
    pub link_path: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MetadataResponse {
    pub file_type: String,
    pub permissions: String,
    pub size: u64,
    pub modified: Option<String>,
    pub path: String,
    /// If this is a symlink, the target path.
    pub symlink_target: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ActionResponse {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/stat",
    tag = "metadata",
    params(StatQuery),
    responses(
        (status = 200, description = "File metadata", body = MetadataResponse),
        (status = 404, description = "File not found"),
    ),
)]
#[instrument(skip(state))]
pub async fn stat(
    State(state): State<AppState>,
    Query(query): Query<StatQuery>,
) -> Result<Json<MetadataResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, "Getting file metadata");

    let metadata = state
        .stat_cache
        .get_or_fetch(&path, || state.vfs.stat(&path))
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("stat"), Some(path.as_path())))?;

    state.vfs_metrics.stat_operations_total.inc();

    Ok(Json(MetadataResponse {
        file_type: metadata.file_type().to_string(),
        permissions: format!("{:o}", metadata.permissions()),
        size: metadata.size,
        modified: metadata
            .modified
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string()),
        path: metadata.path.to_string_lossy().to_string(),
        symlink_target: metadata
            .symlink_target
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/rename",
    tag = "metadata",
    request_body = RenameRequest,
    responses(
        (status = 200, description = "File renamed", body = ActionResponse),
        (status = 404, description = "File not found"),
    ),
)]
#[instrument(skip(state))]
pub async fn rename(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<RenameRequest>,
) -> Result<Json<ActionResponse>, HttpErrorResponse> {
    validation::validate_path(&body.old_path)?;
    validation::validate_path(&body.new_path)?;
    let old_path = PathBuf::from(&body.old_path);
    let new_path = PathBuf::from(&body.new_path);
    info!(old_path = %body.old_path, new_path = %body.new_path, "Renaming file");

    state
        .vfs
        .rename(&old_path, &new_path)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("rename"), Some(old_path.as_path())))?;

    state.stat_cache.invalidate_rename(&old_path, &new_path);
    state.vfs_metrics.rename_operations_total.inc();

    Ok(Json(ActionResponse {
        message: format!("Renamed {} to {}", body.old_path, body.new_path),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/chmod",
    tag = "metadata",
    request_body = ChmodRequest,
    responses(
        (status = 200, description = "Permissions changed", body = ActionResponse),
        (status = 404, description = "File not found"),
    ),
)]
#[instrument(skip(state))]
pub async fn chmod(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<ChmodRequest>,
) -> Result<Json<ActionResponse>, HttpErrorResponse> {
    validation::validate_path(&body.path)?;
    validation::validate_mode(body.mode)?;
    let path = PathBuf::from(&body.path);
    info!(path = %body.path, mode = %body.mode, "Changing file permissions");

    state
        .vfs
        .chmod(&path, body.mode)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("chmod"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);
    state.vfs_metrics.chmod_operations_total.inc();

    Ok(Json(ActionResponse {
        message: format!("Changed permissions of {} to {:o}", body.path, body.mode),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/truncate",
    tag = "metadata",
    request_body = TruncateRequest,
    responses(
        (status = 200, description = "File truncated", body = ActionResponse),
        (status = 404, description = "File not found"),
        (status = 501, description = "Truncate not supported by backend"),
    ),
)]
#[instrument(skip(state))]
pub async fn truncate(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<TruncateRequest>,
) -> Result<Json<ActionResponse>, HttpErrorResponse> {
    validation::validate_path(&body.path)?;
    validation::validate_truncate_size(body.size)?;
    let path = PathBuf::from(&body.path);
    info!(path = %body.path, size = body.size, "Truncating file");

    state
        .vfs
        .truncate(&path, body.size)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("truncate"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);
    state.vfs_metrics.stat_operations_total.inc();

    Ok(Json(ActionResponse {
        message: format!("Truncated {} to {} bytes", body.path, body.size),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/symlink",
    tag = "metadata",
    request_body = SymlinkRequest,
    responses(
        (status = 200, description = "Symlink created", body = ActionResponse),
        (status = 404, description = "Parent not found"),
        (status = 501, description = "Symlink not supported by backend"),
    ),
)]
#[instrument(skip(state))]
pub async fn symlink(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<SymlinkRequest>,
) -> Result<Json<ActionResponse>, HttpErrorResponse> {
    validation::validate_path(&body.target)?;
    validation::validate_path(&body.link_path)?;
    let target = PathBuf::from(&body.target);
    let link_path = PathBuf::from(&body.link_path);
    info!(target = %body.target, link_path = %body.link_path, "Creating symlink");

    state
        .vfs
        .symlink(&target, &link_path)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("symlink"), Some(link_path.as_path())))?;

    state.stat_cache.invalidate(&link_path);
    state.vfs_metrics.stat_operations_total.inc();

    Ok(Json(ActionResponse {
        message: format!("Symlink {} -> {}", body.link_path, body.target),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::files;
    use crate::auth::JwtConfig;
    use axum::Json;
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
    async fn stat_returns_404_when_not_found() {
        let state = test_app_state();
        let query = StatQuery {
            path: "/memfs/nonexistent".to_string(),
        };
        let result = stat(State(state), Query(query)).await;
        let err = match &result {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(
            axum::http::StatusCode::from(err.code.clone()),
            axum::http::StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn stat_returns_metadata_for_file() {
        let state = test_app_state();
        let path = "/memfs/stat_me.txt";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: path.to_string(),
            }),
        )
        .await;

        let query = StatQuery {
            path: path.to_string(),
        };
        let result = stat(State(state), Query(query)).await;
        assert!(result.is_ok());
        let json = result.unwrap().0;
        assert!(json.path.ends_with("stat_me.txt"));
        assert!(json.file_type.to_lowercase().contains("file"));
    }

    #[tokio::test]
    async fn rename_moves_file() {
        let state = test_app_state();
        let old_path = "/memfs/old_name.txt";
        let new_path = "/memfs/new_name.txt";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: old_path.to_string(),
            }),
        )
        .await;

        let body = RenameRequest {
            old_path: old_path.to_string(),
            new_path: new_path.to_string(),
        };
        let result = rename(State(state.clone()), Json(body)).await;
        assert!(result.is_ok());
        let msg = result.unwrap().0.message;
        assert!(msg.contains("Renamed") && msg.contains("new_name.txt"));

        let stat_query = StatQuery {
            path: new_path.to_string(),
        };
        let stat_result = stat(State(state), Query(stat_query)).await;
        assert!(stat_result.is_ok());
    }

    #[tokio::test]
    async fn chmod_changes_permissions() {
        let state = test_app_state();
        let path = "/memfs/chmod_me.txt";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: path.to_string(),
            }),
        )
        .await;

        let body = ChmodRequest {
            path: path.to_string(),
            mode: 0o644,
        };
        let result = chmod(State(state), Json(body)).await;
        assert!(result.is_ok());
        let msg = result.unwrap().0.message;
        assert!(msg.contains("644") || msg.contains("0o644"));
    }

    #[tokio::test]
    async fn chmod_rejects_invalid_mode() {
        let state = test_app_state();
        let body = ChmodRequest {
            path: "/memfs/any".to_string(),
            mode: 0o10000,
        };
        let result = chmod(State(state), Json(body)).await;
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
    async fn truncate_resizes_file() {
        let state = test_app_state();
        let path = "/memfs/trunc_me.txt";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: path.to_string(),
            }),
        )
        .await;
        let _ = files::write_file(
            State(state.clone()),
            axum::Json(files::WriteRequest {
                path: path.to_string(),
                data: "hello world".to_string(),
                offset: Some(0),
            }),
        )
        .await;

        let body = TruncateRequest {
            path: path.to_string(),
            size: 5,
        };
        let result = truncate(State(state.clone()), Json(body)).await;
        assert!(result.is_ok());
        let msg = result.unwrap().0.message;
        assert!(msg.contains("Truncated") && msg.contains("5 bytes"));

        let read_result = files::read_file(
            State(state),
            Query(files::FileQuery {
                path: path.to_string(),
            }),
        )
        .await;
        assert!(read_result.is_ok());
        assert_eq!(read_result.unwrap().0, "hello");
    }

    #[tokio::test]
    async fn symlink_creates_link() {
        let state = test_app_state();
        let target = "/memfs/target.txt";
        let link_path = "/memfs/mylink";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: target.to_string(),
            }),
        )
        .await;

        let body = SymlinkRequest {
            target: target.to_string(),
            link_path: link_path.to_string(),
        };
        let result = symlink(State(state), Json(body)).await;
        assert!(result.is_ok());
        let msg = result.unwrap().0.message;
        assert!(msg.contains("Symlink") && msg.contains("mylink") && msg.contains("target.txt"));
    }
}
