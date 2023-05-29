use axum::http::{Method, StatusCode, Uri};
use axum::response::Html;

pub mod index;

pub mod api;

pub mod downloads;

pub async fn login() -> Html<&'static str> {
    Html(include_str!("../../static/login.html"))
}

pub async fn handle_global_fallback(method: Method, uri: Uri) -> StatusCode {
    tracing::error!(?method, ?uri, "Could not match request with any routers");

    StatusCode::NOT_FOUND
}
