use crate::model::ServiceState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::JsonLines;
use axum_extra::json_lines::AsResponse;
use freighter_index::{CrateVersion, IndexClient};
use metrics::histogram;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use tokio_stream::{Stream, StreamExt};

pub fn index_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: IndexClient + Send + Sync + 'static,
    S: Send + Sync + 'static,
    A: Send + Sync + 'static,
{
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

async fn config<I, S, A>(State(state): State<Arc<ServiceState<I, S, A>>>) -> Json<RegistryConfig> {
    RegistryConfig {
        dl: state.config.service.download_endpoint.clone(),
        api: state.config.service.api_endpoint.clone(),
    }
    .into()
}

async fn get_sparse_meta<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((_, _, crate_name)): Path<(String, String, String)>,
) -> Result<JsonLines<impl Stream<Item = Result<CrateVersion, Infallible>>, AsResponse>, StatusCode>
where
    I: IndexClient,
{
    // todo we can make this streamable, does it get us anything to do so?

    let timer = Instant::now();

    let resp = state
        .index
        .get_sparse_entry(&crate_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .and_then(|crate_versions| crate_versions.ok_or(StatusCode::NOT_FOUND))
        .and_then(|crate_versions| {
            Ok(JsonLines::new(
                tokio_stream::iter(crate_versions).map(|c| Ok(c)),
            ))
        });

    let elapsed = timer.elapsed();

    let code = if let Err(e) = &resp {
        e.as_u16().to_string()
    } else {
        "200".to_string()
    };

    histogram!("request_duration_seconds", elapsed, "code" => code, "endpoint" => "get_sparse");

    resp
}

async fn handle_index_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
