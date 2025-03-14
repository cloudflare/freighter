use axum::body::Body;
use axum::extract::{DefaultBodyLimit, MatchedPath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use freighter_api_types::index::request::ListQuery;
use freighter_api_types::index::response::ListAll;
use freighter_api_types::index::IndexProvider;
use freighter_api_types::storage::StorageProvider;
use freighter_auth::AuthProvider;
use metrics::{counter, histogram};
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

    /// Maximum limit for the size of published crates.
    /// This includes both the metadata and the crate tarball.
    #[serde(default = "default_crate_size_limit")]
    pub crate_size_limit: usize,
}

impl ServiceConfig {
    pub fn sanitize(&mut self) {
        if !self.api_endpoint.contains("://") {
            self.api_endpoint = format!("https://{}", self.api_endpoint);
        }
        if !self.download_endpoint.contains("://") {
            self.download_endpoint = format!("https://{}", self.download_endpoint);
        }
    }
}

pub struct ServiceState {
    pub config: ServiceConfig,
    pub index: Box<dyn IndexProvider + Send + Sync + 'static>,
    pub storage: Box<dyn StorageProvider + Send + Sync + 'static>,
    pub auth: Box<dyn AuthProvider + Send + Sync + 'static>,
}

impl ServiceState {
    #[must_use]
    pub fn new(
        mut config: ServiceConfig,
        index: Box<dyn IndexProvider + Send + Sync + 'static>,
        storage: Box<dyn StorageProvider + Send + Sync + 'static>,
        auth: Box<dyn AuthProvider + Send + Sync + 'static>,
    ) -> Self {
        config.sanitize();
        Self {
            config,
            index,
            storage,
            auth,
        }
    }
}

pub fn router(
    config: ServiceConfig,
    index_client: Box<dyn IndexProvider + Send + Sync + 'static>,
    storage_client: Box<dyn StorageProvider + Send + Sync + 'static>,
    auth_client: Box<dyn AuthProvider + Send + Sync + 'static>,
) -> Router {
    let crate_size_limit = config.crate_size_limit;
    let state = Arc::new(ServiceState::new(
        config,
        index_client,
        storage_client,
        auth_client,
    ));

    Router::new()
        .nest("/downloads", downloads::downloads_router())
        .nest("/index", index::index_router())
        .nest(
            "/api/v1/crates",
            api::api_router().layer(DefaultBodyLimit::max(crate_size_limit)),
        )
        .route("/me", get(register))
        .route("/all", get(list))
        .route("/healthcheck", get(healthcheck))
        .route("/", get(root_page))
        .with_state(state)
        .fallback(handle_global_fallback)
        .layer(CatchPanicLayer::custom(|_| {
            counter!("freighter_panics_total").increment(1);

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
        .layer(from_fn(x_powered_by))
        .layer(from_fn(metrics_layer))
}

async fn metrics_layer(request: Request<Body>, next: Next) -> Response {
    let timer = Instant::now();

    let path = match request.extensions().get::<MatchedPath>() {
        Some(path) => path.as_str().to_string(),
        None => String::default(),
    };

    let response = next.run(request).await;

    let elapsed = timer.elapsed();

    let code = response.status().as_u16().to_string();

    histogram!("freighter_request_duration_seconds", "code" => code, "endpoint" => path)
        .record(elapsed);

    response
}

async fn x_powered_by(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert("x-powered-by", HeaderValue::from_static(concat!("freighter/", env!("CARGO_PKG_VERSION"))));
    response.headers_mut().insert(header::SERVER, HeaderValue::from_static("axum/0.7"));
    response
}

pub async fn root_page(State(state): State<Arc<ServiceState>>) -> String {
    format!(
        "The registry URL for cargo is \"sparse+https://{api}/index\".

This is root of the Freighter server. There's nothing here.
The API endpoint is at {api}.
The download endpoint is at {download}.
Auth is always required: {auth}",
        api = state.config.api_endpoint,
        download = state.config.download_endpoint,
        auth = state.config.auth_required,
    )
}

pub async fn register(State(state): State<Arc<ServiceState>>) -> Html<&'static str> {
    let page = if let Err(why) = state.auth.register_supported() {
        why
    } else {
        include_str!("../static/register.html")
    };
    Html(page)
}

async fn list(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Query(query): Query<ListQuery>,
) -> axum::response::Result<Json<ListAll>> {
    if state.config.auth_required {
        let token = state.auth.token_from_headers(&headers)?.ok_or(StatusCode::UNAUTHORIZED)?;
        state.auth.auth_view_full_index(token).await?;
    }

    let search_results = state.index.list(&query).await?;

    Ok(Json(search_results))
}

async fn healthcheck(State(state): State<Arc<ServiceState>>) -> axum::response::Result<String> {
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

pub async fn handle_global_fallback() -> (StatusCode, &'static str) {
    (
        StatusCode::NOT_FOUND,
        "Freighter: There is no such URL at the root of the server",
    )
}

#[inline(always)]
const fn default_true() -> bool {
    true
}

#[inline(always)]
const fn default_crate_size_limit() -> usize {
    16 * 1024 * 1024
}
