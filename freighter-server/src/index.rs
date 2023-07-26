use crate::ServiceState;
use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::JsonLines;
use axum_extra::json_lines::AsResponse;
use freighter_api_types::index::response::{CrateVersion, RegistryConfig};
use freighter_auth::AuthProvider;
use freighter_index::IndexProvider;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::{Stream, StreamExt};

pub fn index_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: IndexProvider + Send + Sync + 'static,
    S: Send + Sync + 'static,
    A: AuthProvider + Send + Sync + 'static,
{
    Router::new()
        .route("/config.json", get(config))
        .route("/:prefix_1/:prefix_2/:crate_name", get(get_sparse_meta))
        .fallback(handle_index_fallback)
}

async fn config<I, S, A>(State(state): State<Arc<ServiceState<I, S, A>>>) -> Json<RegistryConfig> {
    RegistryConfig {
        dl: state.config.download_endpoint.clone(),
        api: state.config.api_endpoint.clone(),
    }
    .into()
}

async fn get_sparse_meta<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((_, _, crate_name)): Path<(String, String, String)>,
) -> axum::response::Result<
    JsonLines<impl Stream<Item = Result<CrateVersion, Infallible>>, AsResponse>,
>
where
    I: IndexProvider,
    A: AuthProvider + Sync,
{
    let token = headers
        .get(AUTHORIZATION)
        .map(|x| x.to_str().or(Err(StatusCode::BAD_REQUEST)))
        .transpose()?;

    state.auth.auth_index_fetch(token, &crate_name).await?;

    let crate_versions = state.index.get_sparse_entry(&crate_name).await?;

    let resp = JsonLines::new(tokio_stream::iter(crate_versions).map(Ok));

    Ok(resp)
}

async fn handle_index_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
