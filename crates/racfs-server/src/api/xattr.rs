//! Extended attribute (xattr) endpoints

use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    response::Json,
};
use racfs_http_error::HttpErrorResponse;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::api::metadata::ActionResponse;
use crate::error::map_fs_error_with_context;
use crate::state::AppState;
use crate::validation;
use racfs_core::filesystem::FileSystem;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct XattrQuery {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct XattrListQuery {
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct SetXattrRequest {
    pub path: String,
    pub name: String,
    /// Base64-encoded value
    pub value: String,
}

#[derive(Serialize, ToSchema)]
pub struct XattrListResponse {
    pub names: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct XattrValueResponse {
    /// Base64-encoded value
    pub value: String,
}

// Uses metadata::ActionResponse for set_xattr, remove_xattr responses

#[utoipa::path(
    get,
    path = "/api/v1/xattr",
    tag = "xattr",
    params(XattrQuery),
    responses(
        (status = 200, description = "Extended attribute value", body = XattrValueResponse),
        (status = 404, description = "Path or xattr not found"),
        (status = 501, description = "Xattr not supported by backend"),
    ),
)]
#[instrument(skip(state))]
pub async fn get_xattr(
    State(state): State<AppState>,
    Query(query): Query<XattrQuery>,
) -> Result<Json<XattrValueResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, name = %query.name, "Getting xattr");

    let value = state
        .vfs
        .get_xattr(&path, &query.name)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("get_xattr"), Some(path.as_path())))?;

    Ok(Json(XattrValueResponse {
        value: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &value),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/xattr",
    tag = "xattr",
    request_body = SetXattrRequest,
    responses(
        (status = 200, description = "Xattr set", body = ActionResponse),
        (status = 404, description = "Path not found"),
        (status = 501, description = "Xattr not supported by backend"),
    ),
)]
#[instrument(skip(state))]
pub async fn set_xattr(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<SetXattrRequest>,
) -> Result<Json<ActionResponse>, HttpErrorResponse> {
    validation::validate_path(&body.path)?;
    let path = PathBuf::from(&body.path);
    info!(path = %body.path, name = %body.name, "Setting xattr");

    let value = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &body.value)
        .map_err(|_| HttpErrorResponse::bad_request("Invalid base64 value"))?;

    state
        .vfs
        .set_xattr(&path, &body.name, &value)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("set_xattr"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);

    Ok(Json(ActionResponse {
        message: format!("Set xattr {} on {}", body.name, body.path),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/xattr",
    tag = "xattr",
    params(XattrQuery),
    responses(
        (status = 200, description = "Xattr removed", body = ActionResponse),
        (status = 404, description = "Path or xattr not found"),
        (status = 501, description = "Xattr not supported by backend"),
    ),
)]
#[instrument(skip(state))]
pub async fn remove_xattr(
    State(state): State<AppState>,
    Query(query): Query<XattrQuery>,
) -> Result<Json<ActionResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, name = %query.name, "Removing xattr");

    state
        .vfs
        .remove_xattr(&path, &query.name)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("remove_xattr"), Some(path.as_path())))?;

    state.stat_cache.invalidate(&path);

    Ok(Json(ActionResponse {
        message: format!("Removed xattr {} from {}", query.name, query.path),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/xattr/list",
    tag = "xattr",
    params(XattrListQuery),
    responses(
        (status = 200, description = "List of xattr names", body = XattrListResponse),
        (status = 404, description = "Path not found"),
        (status = 501, description = "Xattr not supported by backend"),
    ),
)]
#[instrument(skip(state))]
pub async fn list_xattr(
    State(state): State<AppState>,
    Query(query): Query<XattrListQuery>,
) -> Result<Json<XattrListResponse>, HttpErrorResponse> {
    validation::validate_path(&query.path)?;
    let path = PathBuf::from(&query.path);
    info!(path = %query.path, "Listing xattr");

    let names = state
        .vfs
        .list_xattr(&path)
        .await
        .map_err(|e| map_fs_error_with_context(e, Some("list_xattr"), Some(path.as_path())))?;

    Ok(Json(XattrListResponse { names }))
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
    async fn set_xattr_then_get_xattr() {
        let state = test_app_state();
        let path = "/memfs/xattr_file.txt";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: path.to_string(),
            }),
        )
        .await;

        // "bar" in base64
        let body = SetXattrRequest {
            path: path.to_string(),
            name: "user.test".to_string(),
            value: "YmFy".to_string(),
        };
        let set = set_xattr(State(state.clone()), Json(body)).await;
        assert!(set.is_ok());
        assert!(set.unwrap().0.message.contains("Set xattr"));

        let get_query = XattrQuery {
            path: path.to_string(),
            name: "user.test".to_string(),
        };
        let get = get_xattr(State(state.clone()), Query(get_query)).await;
        assert!(get.is_ok());
        assert_eq!(get.unwrap().0.value, "YmFy");

        let list_query = XattrListQuery {
            path: path.to_string(),
        };
        let list = list_xattr(State(state.clone()), Query(list_query)).await;
        assert!(list.is_ok());
        assert!(list.unwrap().0.names.contains(&"user.test".to_string()));

        let remove_query = XattrQuery {
            path: path.to_string(),
            name: "user.test".to_string(),
        };
        let remove = remove_xattr(State(state.clone()), Query(remove_query)).await;
        assert!(remove.is_ok());

        let get_after = get_xattr(
            State(state),
            Query(XattrQuery {
                path: path.to_string(),
                name: "user.test".to_string(),
            }),
        )
        .await;
        let err = match &get_after {
            Ok(_) => panic!("expected error after remove"),
            Err(e) => e,
        };
        // MemFS returns InvalidInput for "xattr not found" -> 400
        assert_eq!(
            axum::http::StatusCode::from(err.code.clone()),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn set_xattr_rejects_invalid_base64() {
        let state = test_app_state();
        let body = SetXattrRequest {
            path: "/memfs/any".to_string(),
            name: "user.foo".to_string(),
            value: "not-valid-base64!!!".to_string(),
        };
        let result = set_xattr(State(state), Json(body)).await;
        let err = match &result {
            Ok(_) => panic!("expected bad request"),
            Err(e) => e,
        };
        assert_eq!(
            axum::http::StatusCode::from(err.code.clone()),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn get_xattr_returns_error_when_name_missing() {
        let state = test_app_state();
        let path = "/memfs/has_one_attr.txt";
        let _ = files::create_file(
            State(state.clone()),
            Query(files::FileQuery {
                path: path.to_string(),
            }),
        )
        .await;
        let set_body = SetXattrRequest {
            path: path.to_string(),
            name: "user.present".to_string(),
            value: "YQ==".to_string(), // "a" in base64
        };
        let _ = set_xattr(State(state.clone()), Json(set_body)).await;

        let query = XattrQuery {
            path: path.to_string(),
            name: "user.missing".to_string(),
        };
        let result = get_xattr(State(state), Query(query)).await;
        let err = match &result {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        // MemFS returns InvalidInput for missing xattr name -> 400
        assert_eq!(
            axum::http::StatusCode::from(err.code.clone()),
            axum::http::StatusCode::BAD_REQUEST
        );
    }
}
