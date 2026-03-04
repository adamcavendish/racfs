//! File operation endpoints

use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    response::Json,
};
use racfs_http_error::HttpErrorResponse;
use serde::Deserialize;
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::error::map_fs_error_with_context;
use crate::state::AppState;
use crate::validation;
use racfs_core::filesystem::{DirFS, ReadFS, WriteFS};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct FileQuery {
    pub path: String,
}

#[derive(Debug, Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct WriteRequest {
    pub path: String,
    pub data: String,
    pub offset: Option<i64>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct FileResponse {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/files",
    tag = "files",
    params(FileQuery),
    responses(
        (status = 200, description = "File content", body = String),
        (status = 404, description = "File not found"),
        (status = 400, description = "Invalid request"),
    ),
)]
#[instrument(skip(state))]
pub async fn read_file(
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
) -> Result<Json<String>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, "Reading file");

    let data = state
        .vfs
        .read(&path, 0, -1)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("read"), Some(path.as_path())))?;

    state.vfs_metrics.file_reads_total.inc();

    let content = String::from_utf8_lossy(&data).to_string();
    Ok(Json(content))
}

#[utoipa::path(
    post,
    path = "/api/v1/files",
    tag = "files",
    params(FileQuery),
    responses(
        (status = 200, description = "File created", body = FileResponse),
        (status = 409, description = "File already exists"),
    ),
)]
#[instrument(skip(state))]
pub async fn create_file(
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
) -> Result<Json<FileResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, "Creating file");

    state
        .vfs
        .create(&path)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("create"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);
    state.vfs_metrics.file_writes_total.inc();

    Ok(Json(FileResponse {
        message: format!("Created file: {}", query.path),
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/files",
    tag = "files",
    request_body = WriteRequest,
    responses(
        (status = 200, description = "File written", body = FileResponse),
        (status = 404, description = "File not found"),
    ),
)]
#[instrument(skip(state))]
pub async fn write_file(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<WriteRequest>,
) -> Result<Json<FileResponse>, HttpErrorResponse> {
    validation::validate_path(&body.path)?;
    validation::validate_write_data_len(body.data.len())?;
    let path = PathBuf::from(&body.path);
    let offset = body.offset.unwrap_or(0);
    let data = body.data.as_bytes();
    info!(path = %body.path, offset = offset, data_len = data.len(), "Writing file");

    let written = state
        .vfs
        .write(&path, data, offset, racfs_core::flags::WriteFlags::none())
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("write"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);
    state.vfs_metrics.file_writes_total.inc();

    Ok(Json(FileResponse {
        message: format!("Wrote {} bytes", written),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/files",
    tag = "files",
    params(FileQuery),
    responses(
        (status = 200, description = "File deleted", body = FileResponse),
        (status = 404, description = "File not found"),
    ),
)]
#[instrument(skip(state))]
pub async fn delete_file(
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
) -> Result<Json<FileResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, "Deleting file");

    state
        .vfs
        .remove(&path)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("remove"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);
    state.vfs_metrics.file_deletes_total.inc();

    Ok(Json(FileResponse {
        message: format!("Deleted: {}", query.path),
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
    async fn read_file_returns_404_when_not_found() {
        let state = test_app_state();
        let query = FileQuery {
            path: "/memfs/nonexistent.txt".to_string(),
        };
        let result = read_file(State(state), Query(query)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            axum::http::StatusCode::from(err.code),
            axum::http::StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn read_file_rejects_invalid_path() {
        let state = test_app_state();
        let query = FileQuery {
            path: "relative/path".to_string(),
        };
        let result = read_file(State(state), Query(query)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            axum::http::StatusCode::from(err.code),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn create_file_then_read_file() {
        let state = test_app_state();
        let path = "/memfs/unit_test.txt";

        let create_query = FileQuery {
            path: path.to_string(),
        };
        let create = create_file(State(state.clone()), Query(create_query)).await;
        assert!(create.is_ok(), "{:?}", create.err());
        let msg = create.unwrap();
        assert!(msg.0.message.contains("Created file") && msg.0.message.contains("unit_test.txt"));

        let read_query = FileQuery {
            path: path.to_string(),
        };
        let read = read_file(State(state), Query(read_query)).await;
        assert!(read.is_ok());
        assert_eq!(read.unwrap().0, "");
    }

    #[tokio::test]
    async fn write_file_then_read_file() {
        let state = test_app_state();
        let path = "/memfs/write_unit.txt";

        let _ = create_file(
            State(state.clone()),
            Query(FileQuery {
                path: path.to_string(),
            }),
        )
        .await;

        let body = WriteRequest {
            path: path.to_string(),
            data: "hello from unit test".to_string(),
            offset: Some(0),
        };
        let write = write_file(State(state.clone()), axum::Json(body)).await;
        assert!(write.is_ok());
        assert!(write.unwrap().0.message.contains("20 bytes"));

        let read = read_file(
            State(state),
            Query(FileQuery {
                path: path.to_string(),
            }),
        )
        .await;
        assert!(read.is_ok());
        assert_eq!(read.unwrap().0, "hello from unit test");
    }

    #[tokio::test]
    async fn delete_file_removes_file() {
        let state = test_app_state();
        let path = "/memfs/delete_me.txt";

        let _ = create_file(
            State(state.clone()),
            Query(FileQuery {
                path: path.to_string(),
            }),
        )
        .await;

        let del = delete_file(
            State(state.clone()),
            Query(FileQuery {
                path: path.to_string(),
            }),
        )
        .await;
        assert!(del.is_ok());

        let read = read_file(
            State(state),
            Query(FileQuery {
                path: path.to_string(),
            }),
        )
        .await;
        assert!(read.is_err());
        assert_eq!(
            axum::http::StatusCode::from(read.unwrap_err().code),
            axum::http::StatusCode::NOT_FOUND
        );
    }
}
