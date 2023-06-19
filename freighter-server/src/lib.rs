use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{Request, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use freighter_auth::AuthClient;
use freighter_index::IndexClient;
use freighter_storage::StorageClient;
use metrics::{histogram, increment_counter};
use std::sync::Arc;
use std::time::Instant;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::classify::StatusInRangeAsFailures;
use tower_http::trace::{DefaultOnFailure, TraceLayer};

pub use config::ServiceConfig;

mod config;
mod model;
mod routes;

pub fn router<I, S, A>(
    config: ServiceConfig,
    index_client: I,
    storage_client: S,
    auth_client: A,
) -> Router
where
    I: IndexClient + Send + Sync + 'static,
    S: StorageClient + Clone + Send + Sync + 'static,
    A: AuthClient + Send + Sync + 'static,
{
    let state = Arc::new(model::ServiceState::new(
        config,
        index_client,
        storage_client,
        auth_client,
    ));

    Router::new()
        .nest("/downloads", routes::downloads::downloads_router())
        .nest("/index", routes::index::index_router())
        .nest("/api/v1/crates", routes::api::api_router())
        .route("/me", get(routes::login))
        .with_state(state)
        .fallback(routes::handle_global_fallback)
        .layer(CatchPanicLayer::custom(|_| {
            increment_counter!("panics_total");

            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
        .layer(
            TraceLayer::new(StatusInRangeAsFailures::new(400..=599).into_make_classifier())
                .make_span_with(|request: &Request<Body>| {
                    // todo don't log stuff which may have sensitive information

                    let method = request.method();
                    let uri = request.uri();
                    let headers = request.headers();

                    tracing::info_span!("http-request", ?method, ?uri, ?headers)
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
