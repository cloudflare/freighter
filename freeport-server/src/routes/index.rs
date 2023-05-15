use crate::model::types::index::CrateVersion;
use crate::model::ServiceState;
use axum::extract::{Path, State};
use axum::http::{Method, StatusCode, Uri};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::JsonLines;
use axum_extra::json_lines::AsResponse;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::{Stream, StreamExt};

pub fn index_router() -> Router<Arc<ServiceState>> {
    Router::new()
        .route("/config.json", get(config))
        .route("/:prefix_1/:prefix_2/:crate_name", get(get_sparse_meta))
        .fallback(handle_index_fallback)
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
struct RegistryConfig {
    dl: String,
    api: String,
}

async fn config(State(state): State<Arc<ServiceState>>) -> Json<RegistryConfig> {
    RegistryConfig {
        dl: state.config.service.download_endpoint.clone(),
        api: state.config.service.api_endpoint.clone(),
    }
    .into()
}

async fn get_sparse_meta(
    State(state): State<Arc<ServiceState>>,
    Path((_, _, crate_name)): Path<(String, String, String)>,
) -> Result<JsonLines<impl Stream<Item = Result<CrateVersion, Infallible>>, AsResponse>, StatusCode>
{
    // todo we can make this streamable, does it get us anything to do so?

    let crate_versions = state
        .get_sparse_metadata(&crate_name)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(JsonLines::new(
        tokio_stream::iter(crate_versions).map(|c| Ok(c)),
    ))
}

async fn handle_index_fallback(method: Method, uri: Uri) -> StatusCode {
    tracing::error!(
        ?method,
        ?uri,
        "Could not match request with any routes on index"
    );

    StatusCode::NOT_FOUND
}
