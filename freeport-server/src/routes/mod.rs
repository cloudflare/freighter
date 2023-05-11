use axum::http::{Method, StatusCode, Uri};

pub mod index;

pub mod api;

pub async fn handle_global_fallback(method: Method, uri: Uri) -> StatusCode {
    tracing::error!(?method, ?uri, "Could not match request with any routers");

    StatusCode::NOT_FOUND
}
