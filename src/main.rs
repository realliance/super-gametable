mod cli;
mod config;
mod http;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use config::Config;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();
    info!("It's-a Super Gametable!");

    info!("Loading configuration from environment variables");
    let config = Config::try_from_env()?;

    Ok(())
}
