use anyhow::Context;
use clap::Parser;
use freighter_auth::pg_backend::PgAuthProvider;
use freighter_index::postgres_client::PgIndexProvider;
use freighter_storage::s3_client::S3StorageProvider;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::fs::read_to_string;
use tokio::signal;

mod cli;
mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = cli::FreighterArgs::parse();

    let config: config::Config = serde_yaml::from_str(
        &read_to_string(args.config)
            .context("Failed to read config file from disk, is it present?")?,
    )
    .context("Failed to deserialize config file, please make sure its in the right format")?;

    let config::Config {
        service,
        index_db,
        auth_db,
        store,
    } = config;

    PrometheusBuilder::new()
        .add_global_label("service", "freighter")
        .with_http_listener(service.metrics_address)
        .set_buckets(&[
            100e-6, 500e-6, 1e-3, 5e-3, 1e-2, 5e-2, 1e-1, 2e-1, 3e-1, 4e-1, 5e-1, 6e-1, 7e-1, 8e-1,
            9e-1, 1.0, 5.0, 10.0,
        ])
        .context("Failed to set buckets for prometheus")?
        .install()
        .context("Failed to install prometheus exporter")?;

    let addr = service.address;

    let index_client =
        PgIndexProvider::new(index_db).context("Failed to construct index client")?;
    let storage_client = S3StorageProvider::new(
        &store.name,
        &store.endpoint_url,
        &store.region,
        &store.access_key_id,
        &store.access_key_secret,
    );
    let auth_client = PgAuthProvider::new(auth_db).context("Failed to initialize auth client")?;

    let router = freighter_server::router(service, index_client, storage_client, auth_client);

    tracing::info!(?addr, "Starting freighter instance");

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Freighter server exited with error")
}

// Based on: https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    terminate.await;

    println!("SIGTERM received, shutting down...");
}
