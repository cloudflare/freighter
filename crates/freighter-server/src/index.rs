use crate::ServiceState;
use axum::extract::{Path, State};
use axum::http::header::WWW_AUTHENTICATE;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::JsonLines;
use axum_extra::json_lines::AsResponse;
use freighter_api_types::index::response::{CrateVersion, RegistryConfig};
use freighter_api_types::index::IndexProvider;
use freighter_auth::AuthProvider;
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::{Stream, StreamExt};

const CARGO_AUTH_REQUIRED_ERROR: &str = "error: This registry requires `cargo +nightly -Z registry-auth` or `CARGO_UNSTABLE_REGISTRY_AUTH=true RUSTC_BOOTSTRAP=1`";

pub fn index_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: IndexProvider + Send + Sync + 'static,
    S: Send + Sync + 'static,
    A: AuthProvider + Send + Sync + 'static,
{
    Router::new()
        .route("/config.json", get(config))
        .route("/*crate_index_path", get(get_sparse_meta))
        .fallback(handle_index_fallback)
}

async fn config<I, S, A>(headers: HeaderMap, State(state): State<Arc<ServiceState<I, S, A>>>) -> axum::response::Result<Json<RegistryConfig>>
where
    A: AuthProvider + Send + Sync + 'static,
{
    let auth_required = state.config.auth_required;
    if auth_required {
        let Some(token) = state.auth.token_from_headers(&headers)? else {
            return Err((StatusCode::UNAUTHORIZED, [(WWW_AUTHENTICATE, "Cargo login_url=/me")], CARGO_AUTH_REQUIRED_ERROR).into());
        };
        state.auth.auth_config(token).await?;
    }

    Ok(RegistryConfig {
        dl: state.config.download_endpoint.clone(),
        api: state.config.api_endpoint.clone(),
        auth_required,
    }
    .into())
}

async fn get_sparse_meta<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(crate_index_path): Path<String>,
) -> axum::response::Result<
    JsonLines<impl Stream<Item = Result<CrateVersion, Infallible>>, AsResponse>,
>
where
    I: IndexProvider,
    A: AuthProvider + Sync,
{
    let Some((_, crate_name)) = crate_index_path.rsplit_once('/') else {
        tracing::warn!("Received index request with no path");
        return Err(StatusCode::BAD_REQUEST.into());
    };

    if state.config.auth_required {
        let token = state.auth.token_from_headers(&headers)?.ok_or(StatusCode::UNAUTHORIZED)?;
        state.auth.auth_index_fetch(token, crate_name).await?;
    }

    let mut crate_versions = state.index.get_sparse_entry(crate_name).await?;
    // Fixes already-published crates
    ensure_correct_metadata(&mut crate_versions);

    let resp = JsonLines::new(tokio_stream::iter(crate_versions).map(Ok));

    Ok(resp)
}


fn ensure_correct_metadata(entries: &mut [CrateVersion]) {
    for e in entries {
        let valid_features: HashSet<_> = e.features.keys().chain(e.features2.keys()).cloned().collect();
        let mut missing_features = Vec::new();
        for feature_actions in e.features.values_mut().chain(e.features2.values_mut()) {
            // ensure there are no features that refer to non-existent dependencies - cargo checks for this.
            // (this may happen because dev deps are not in the index)
            feature_actions.retain(|action| {
                if !valid_features.contains(action) {
                    let action = action.strip_prefix("dep:").unwrap_or(action).split(['?','/']).next().unwrap();
                    if !e.deps.iter().any(|d| d.name == action) {
                        if !valid_features.contains(action) {
                            missing_features.push(action.to_string());
                        }
                        return false;
                    }
                }
                true
            });
        }
        // make the features exist in case other crates refer to them too
        for f in missing_features {
            e.features.entry(f).or_insert_with(Vec::new);
        }
    }
}

async fn handle_index_fallback() -> (StatusCode, &'static str) {
    (
        StatusCode::NOT_FOUND,
        "Freighter: Invalid URL for the crate index endpoint",
    )
}
