use axum::body::Body;
use axum::extract::{MatchedPath, Query, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use freighter_api_types::index::request::ListQuery;
use freighter_api_types::index::response::ListAll;
use freighter_api_types::index::IndexProvider;
use freighter_api_types::storage::StorageProvider;
use freighter_auth::AuthProvider;
use metrics::{histogram, counter};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tokio::try_join;
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
    #[serde(default = "default_true")]
    pub allow_registration: bool,

    /// Use auth for all requests to the registry, including config and index.
    /// Currently requires `-Z registry-auth` nightly feature.
    #[serde(default = "default_true")]
    pub auth_required: bool,
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
        .route("/me", get(register))
        .route("/all", get(list))
        .route("/healthcheck", get(healthcheck))
        .with_state(state)
        .fallback(handle_global_fallback)
        .layer(CatchPanicLayer::custom(|_| {
            counter!("panics_total").increment(1);

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

    histogram!("request_duration_seconds", "code" => code, "endpoint" => path)
        .record(elapsed);

    response
}

pub async fn register() -> Html<&'static str> {
    Html(include_str!("../static/register.html"))
}

async fn list<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Query(query): Query<ListQuery>,
) -> axum::response::Result<Json<ListAll>>
where
    I: IndexProvider,
    A: AuthProvider + Sync,
{
    if state.config.auth_required {
        let token = state.auth.token_from_headers(&headers)?.ok_or(StatusCode::UNAUTHORIZED)?;
        state.auth.auth_view_full_index(token).await?;
    }

    let search_results = state.index.list(&query).await?;

    Ok(Json(search_results))
}

async fn healthcheck<I, S, A>(State(state): State<Arc<ServiceState<I, S, A>>>) -> axum::response::Result<String>
where
    I: IndexProvider,
    S: StorageProvider,
    A: AuthProvider + Sync,
{
    let check_time = Duration::from_secs(4);
    let label = |label, res: Result<Result<(), anyhow::Error>, _>| match res {
        // healthcheck is unauthenticated and shouldn't leak internals via errors
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            for e in e.chain() {
                tracing::error!("{label} healthcheck: {e}");
            }
            Err(format!("{label} failed"))
        },
        Err(_) => Err(format!("{label} timed out")),
    };

    try_join! {
        async { label("auth", timeout(check_time, state.auth.healthcheck()).await) },
        async { label("index", timeout(check_time, state.index.healthcheck()).await) },
        async { label("storage", timeout(check_time, state.storage.healthcheck()).await) },
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok("OK".into())
}

pub async fn handle_global_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}

#[inline(always)]
fn default_true() -> bool {
    true
}

