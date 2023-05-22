use crate::model::ServiceState;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode, Uri};
use axum::routing::get;
use axum::Router;
use semver::Version;
use std::sync::Arc;

pub fn downloads_router() -> Router<Arc<ServiceState>> {
    Router::new()
        .route("/:name/:version", get(serve_crate))
        .fallback(handle_downloads_fallback)
}

async fn serve_crate(
    State(state): State<Arc<ServiceState>>,
    Path((name, version)): Path<(String, Version)>,
) -> Result<Bytes, StatusCode> {
    if let Some(bytes) = state
        .download_crate(&format!("{name}-{version}.crate"))
        .await
    {
        Ok(Bytes::from(bytes))
    } else {
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
