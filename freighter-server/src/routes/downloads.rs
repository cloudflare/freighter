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

pub fn downloads_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: Send + Sync + 'static,
    S: StorageClient + Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    Router::new()
        .route("/:name/:version", get(serve_crate))
        .fallback(handle_downloads_fallback)
}

async fn serve_crate<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<Bytes>
where
    S: StorageClient,
{
    let timer = Instant::now();

    let crate_bytes = state
        .storage
        .pull_crate(&name, &version.to_string())
        .await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "download_crate");

    Ok(Bytes::from(crate_bytes))
}

async fn handle_downloads_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
