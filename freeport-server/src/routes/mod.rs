use axum::http::StatusCode;
use axum::response::Html;

pub mod index;

pub mod api;

pub mod downloads;

pub async fn login() -> Html<&'static str> {
    Html(include_str!("../../static/login.html"))
}

pub async fn handle_global_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
