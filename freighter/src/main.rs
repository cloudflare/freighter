use clap::Parser;
use freighter_auth::yes_backend::YesAuthClient;
use freighter_index::postgres_client::PostgreSQLIndex;
use freighter_storage::s3_client::S3StorageClient;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::fs::read_to_string;

mod cli;
mod config;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = cli::FreighterArgs::parse();

    let config: config::Config =
        serde_yaml::from_str(&read_to_string(args.config).unwrap()).unwrap();

    PrometheusBuilder::new()
        .add_global_label("service", "freighter")
        .with_http_listener(config.service.metrics_address)
        .set_buckets(&[
            100e-6, 500e-6, 1e-3, 5e-3, 1e-2, 5e-2, 1e-1, 2e-1, 3e-1, 4e-1, 5e-1, 6e-1, 7e-1, 8e-1,
            9e-1, 1.0, 5.0, 10.0,
        ])
        .unwrap()
        .install()
        .unwrap();

    let addr = config.service.address;

    let index_client = PostgreSQLIndex::new(config.db).unwrap();
    let storage_client = S3StorageClient::new(
        &config.store.name,
        config.store.region,
        config.store.credentials,
    )
    .unwrap();
    let auth_client = YesAuthClient;

    let router =
        freighter_server::router(config.service, index_client, storage_client, auth_client);

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap()
}
