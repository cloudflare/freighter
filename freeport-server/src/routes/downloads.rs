use crate::model::ServiceState;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use freighter_storage::StorageClient;
use metrics::histogram;
use semver::Version;
use std::sync::Arc;
use std::time::Instant;

pub fn downloads_router<I, S>() -> Router<Arc<ServiceState<I, S>>>
where
    I: Send + Sync + 'static,
    S: StorageClient + Send + Sync + 'static,
{
    Router::new()
        .route("/:name/:version", get(serve_crate))
        .fallback(handle_downloads_fallback)
}

async fn serve_crate<I, S>(
    State(state): State<Arc<ServiceState<I, S>>>,
    Path((name, version)): Path<(String, Version)>,
) -> Result<Bytes, StatusCode>
where
    S: StorageClient,
{
    let timer = Instant::now();

    let resp = state.storage.pull_crate(&name, &version.to_string()).await;

    let elapsed = timer.elapsed();

    if let Ok(bytes) = resp {
        histogram!("request_duration_seconds", elapsed, "code" => "200", "endpoint" => "download_crate");
        Ok(Bytes::from(bytes))
    } else {
        histogram!("request_duration_seconds", elapsed, "code" => "404", "endpoint" => "download_crate");
        Err(StatusCode::NOT_FOUND)
    }
}

async fn handle_downloads_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
