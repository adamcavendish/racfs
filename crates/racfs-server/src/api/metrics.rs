//! Metrics API endpoint for Prometheus

use axum::{
    extract::State,
    http::{StatusCode, header::CONTENT_TYPE},
    response::IntoResponse,
};
use prometheus::Encoder;

use crate::state::AppState;

/// Prometheus metrics endpoint
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "Metrics",
    responses(
        (status = 200, description = "Prometheus metrics", body = String)
    )
)]
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&state.metrics_registry.gather(), &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to encode metrics",
        ));
    }

    let response = String::from_utf8(buffer).unwrap_or_default();

    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        response,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::JwtConfig;
    use crate::state::AppState;
    use racfs_vfs::MountableFS;
    use std::sync::Arc;

    #[tokio::test]
    async fn metrics_returns_200_with_valid_registry() {
        let vfs = Arc::new(MountableFS::new());
        let state = AppState::new(vfs, JwtConfig::default(), 2, vec![]).unwrap();
        let result = metrics(axum::extract::State(state)).await;
        let response = result.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let ct = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());
        assert_eq!(ct, Some("text/plain; version=0.0.4; charset=utf-8"));
    }
}
