use axum::Router;
use clap::Parser;
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

    let addr = config.service.address;

    let state = Arc::new(model::ServiceState::new(config));

    let router = Router::new()
        .nest("/downloads", routes::downloads::downloads_router())
        .nest("/index", routes::index::index_router())
        .nest("/api/v1/crates", routes::api::api_router())
        .with_state(state)
        .fallback(routes::handle_global_fallback)
        .layer(CatchPanicLayer::new());

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap()
}
