use anyhow::Context;

cfg_if::cfg_if! {
    if #[cfg(feature = "filesystem-index-backend")] {
        use freighter_fs_index::FsIndexProvider as SelectedIndexProvider;
    } else if #[cfg(feature = "postgresql-index-backend")] {
        use freighter_pg_index::PgIndexProvider as SelectedIndexProvider;
    } else {
        compile_error!("Use cargo features to select an index backend");
    }
}
cfg_if::cfg_if! {
    if #[cfg(feature = "filesystem-auth-backend")] {
        use freighter_auth::fs_backend::FsAuthProvider as SelectedAuthProvider;
    } else if #[cfg(feature = "cloudflare-auth-backend")] {
        use freighter_auth::cf_backend::CfAuthProvider as SelectedAuthProvider;
    } else if #[cfg(feature = "yes-auth-backend")] {
        use freighter_auth::yes_backend::YesAuthProvider as SelectedAuthProvider;
    } else {
        use freighter_auth::no_backend::NoAuthProvider as SelectedAuthProvider;
    }
}
use freighter_storage::s3_client::S3StorageProvider;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::fs::read_to_string;
use tokio::net::TcpListener;

pub mod cli;
mod config;

pub async fn run(args: cli::FreighterArgs) -> anyhow::Result<()> {
    let config: config::Config<SelectedIndexProvider, SelectedAuthProvider> = serde_yaml::from_str(
        &read_to_string(args.config)
            .context("Failed to read config file from disk, is it present?")?,
    )
    .context("Failed to deserialize config file, please make sure its in the right format")?;

    let config::Config {
        service,
        index_config,
        auth_config,
        store,
    } = config;

    PrometheusBuilder::new()
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
        SelectedIndexProvider::new(index_config).context("Failed to construct index client")?;

    let storage_client = S3StorageProvider::new(
        &store.name,
        &store.endpoint_url,
        &store.region,
        &store.access_key_id.unwrap_or_else(|| {
            std::env::var("FREIGHTER_STORE_BUCKET_KEY_ID")
                .expect("Failed to find store bucket key id in environment variable or config")
        }),
        &store.access_key_secret.unwrap_or_else(|| {
            std::env::var("FREIGHTER_STORE_BUCKET_KEY_SECRET")
                .expect("Failed to find store bucket key secret in environment variable or config")
        }),
    );
    let auth_client =
        SelectedAuthProvider::new(auth_config).context("Failed to initialize auth client")?;

    let router = freighter_server::router(
        service,
        Box::new(index_client),
        Box::new(storage_client),
        Box::new(auth_client),
    );

    tracing::info!(
        ?addr,
        "Starting freighter instance with {} index and {} auth",
        std::any::type_name::<SelectedIndexProvider>(),
        std::any::type_name::<SelectedAuthProvider>()
    );

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Freighter server exited with error")?;

    tracing::info!("Completed graceful shutdown");

    Ok(())
}

// Based on: https://github.com/tokio-rs/axum/blob/main/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal;

        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    terminate.await;

    tracing::info!("SIGTERM received, beginning graceful shutdown");
}
