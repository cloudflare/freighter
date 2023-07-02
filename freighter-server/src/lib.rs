use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{Request, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use freighter_auth::AuthProvider;
use freighter_index::IndexProvider;
use freighter_storage::StorageProvider;
use metrics::{histogram, increment_counter};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::classify::StatusInRangeAsFailures;
use tower_http::trace::{DefaultOnFailure, TraceLayer};

pub mod index;

pub mod api;

pub mod downloads;

#[derive(Clone, Deserialize)]
pub struct ServiceConfig {
    pub address: SocketAddr,
    pub download_endpoint: String,
    pub api_endpoint: String,
    pub metrics_address: SocketAddr,
}

pub struct ServiceState<I, S, A> {
    pub config: ServiceConfig,
    pub index: I,
    pub storage: S,
    pub auth: A,
}

impl<I, S, A> ServiceState<I, S, A> {
    pub fn new(config: ServiceConfig, index: I, storage: S, auth: A) -> Self {
        Self {
            config,
            index,
            storage,
            auth,
        }
    }
}

pub fn router<I, S, A>(
    config: ServiceConfig,
    index_client: I,
    storage_client: S,
    auth_client: A,
) -> Router
    where
        I: IndexProvider + Send + Sync + 'static,
        S: StorageProvider + Clone + Send + Sync + 'static,
        A: AuthProvider + Send + Sync + 'static,
{
    let state = Arc::new(ServiceState::new(
        config,
        index_client,
        storage_client,
        auth_client,
    ));

    Router::new()
        .nest("/downloads", downloads::downloads_router())
        .nest("/index", index::index_router())
        .nest("/api/v1/crates", api::api_router())
        .route("/me", get(login))
        .with_state(state)
        .fallback(handle_global_fallback)
        .layer(CatchPanicLayer::custom(|_| {
            increment_counter!("panics_total");

            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
        .layer(
            TraceLayer::new(StatusInRangeAsFailures::new(400..=599).into_make_classifier())
                .make_span_with(|request: &Request<Body>| {
                    let method = request.method();
                    let uri = request.uri();

                    tracing::info_span!("http-request", ?method, ?uri)
                })
                .on_failure(DefaultOnFailure::new()),
        )
        .layer(from_fn(metrics_layer))
}

async fn metrics_layer<B>(request: Request<B>, next: Next<B>) -> Response {
    let timer = Instant::now();

    let path = if let Some(path) = request.extensions().get::<MatchedPath>() {
        path.as_str().to_string()
    } else {
        request.uri().path().to_string()
    };

    let response = next.run(request).await;

    let elapsed = timer.elapsed();

    let code = response.status().as_u16().to_string();

    histogram!("request_duration_seconds", elapsed, "code" => code, "endpoint" => path);

    response
}

pub async fn login() -> Html<&'static str> {
    Html(include_str!("../static/login.html"))
}

pub async fn handle_global_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
