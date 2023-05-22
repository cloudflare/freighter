use crate::model::ServiceState;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode, Uri};
use axum::routing::get;
use axum::Router;
use metrics::histogram;
use semver::Version;
use std::sync::Arc;
use std::time::Instant;

pub fn downloads_router() -> Router<Arc<ServiceState>> {
    Router::new()
        .route("/:name/:version", get(serve_crate))
        .fallback(handle_downloads_fallback)
}

async fn serve_crate(
    State(state): State<Arc<ServiceState>>,
    Path((name, version)): Path<(String, Version)>,
) -> Result<Bytes, StatusCode> {
    let timer = Instant::now();

    let resp = state
        .download_crate(&format!("{name}-{version}.crate"))
        .await;

    let elapsed = timer.elapsed();

    if let Some(bytes) = resp {
        histogram!("request_duration_seconds", elapsed, "code" => "200", "endpoint" => "download_crate");
        Ok(Bytes::from(bytes))
    } else {
        histogram!("request_duration_seconds", elapsed, "code" => "404", "endpoint" => "download_crate");
        Err(StatusCode::NOT_FOUND)
    }
}

async fn handle_downloads_fallback(method: Method, uri: Uri) -> StatusCode {
    tracing::error!(
        ?method,
        ?uri,
        "Could not match request with any routes on index"
    );

    StatusCode::NOT_FOUND
}
