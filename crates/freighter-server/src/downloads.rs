use crate::ServiceState;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
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
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<axum::response::Response> {
    if state.config.auth_required {
        let token = state
            .auth
            .token_from_headers(&headers)?
            .ok_or(StatusCode::UNAUTHORIZED)?;
        state.auth.auth_crate_download(token, &name).await?;
    }

    let _is_yanked = state.index.confirm_existence(&name, &version).await?;

    let crate_res = state
        .storage
        .pull_crate(&name, &version.to_string())
        .await?;

    let mut res = axum::response::Response::new(crate_res.data.into());
    if let Some(last_mod) = crate_res.last_modified.and_then(|d| d.to_rfc2822().try_into().ok()) {
        res.headers_mut().insert(header::LAST_MODIFIED, last_mod);
    }

    Ok(res)
}

async fn handle_downloads_fallback() -> (StatusCode, &'static str) {
    (
        StatusCode::NOT_FOUND,
        "Freighter: Invalid URL for the crate download endpoint",
    )
}
