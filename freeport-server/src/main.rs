use axum::http::StatusCode;
use axum::middleware::map_response;
use axum::response::{IntoResponse, Response};
use axum::Router;
use clap::Parser;
use metrics::increment_counter;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::fs::read_to_string;
use std::sync::Arc;
use tower_http::catch_panic::CatchPanicLayer;

mod cli;
mod config;
mod model;
mod routes;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = cli::FreePortArgs::parse();

    let config: config::Config =
        serde_yaml::from_str(&read_to_string(args.config).unwrap()).unwrap();

    PrometheusBuilder::new()
        .add_global_label("service", "freeport-server")
        .with_http_listener(config.service.metrics_address)
        .set_buckets(&[
            100e-6, 500e-6, 1e-3, 5e-3, 1e-2, 5e-2, 1e-1, 2e-1, 3e-1, 4e-1, 5e-1, 6e-1, 7e-1, 8e-1,
            9e-1, 1.0, 5.0, 10.0,
        ])
        .unwrap()
        .install()
        .unwrap();

    let addr = config.service.address;

    let state = Arc::new(model::ServiceState::new(config));

    let router = Router::new()
        .nest("/downloads", routes::downloads::downloads_router())
        .nest("/index", routes::index::index_router())
        .nest("/api/v1/crates", routes::api::api_router())
        .with_state(state)
        .fallback(routes::handle_global_fallback)
        .layer(CatchPanicLayer::custom(|_| {
            increment_counter!("panics_total");

            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
        .layer(map_response(record_status_code));

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap()
}

async fn record_status_code<B>(response: Response<B>) -> Response<B> {
    let code = response.status().as_str().to_string();
    increment_counter!("responses_total", "code" => code);
    response
}
