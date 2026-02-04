use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = freighter::cli::FreighterArgs::parse();

    freighter::start_listening(args).await?.await
}
