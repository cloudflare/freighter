use crate::{token_from_headers, ServiceState};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::Router;
use freighter_api_types::index::IndexProvider;
use freighter_auth::AuthProvider;
use freighter_api_types::storage::StorageProvider;
use semver::Version;
use std::sync::Arc;

pub fn downloads_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: IndexProvider + Send + Sync + 'static,
    S: StorageProvider + Send + Sync + 'static,
    A: AuthProvider + Send + Sync + 'static,
{
    Router::new()
        .route("/:name/:version", get(serve_crate))
        .fallback(handle_downloads_fallback)
}

async fn serve_crate<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<Bytes>
where
    I: IndexProvider,
    S: StorageProvider,
    A: AuthProvider + Sync,
{
    if state.config.auth_required {
        let token = token_from_headers(&headers)?;
        state.auth.auth_crate_download(token, &name).await?;
    }

    let _is_yanked = state.index.confirm_existence(&name, &version).await?;

    let crate_bytes = state
        .storage
        .pull_crate(&name, &version.to_string())
        .await?;

    Ok(crate_bytes)
}

async fn handle_downloads_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
