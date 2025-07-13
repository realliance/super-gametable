mod cli;
mod config;
mod controllers;
mod game;
mod game_pool;
mod queue;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, Tool};
use config::Config;
use game_pool::{GamePool, GamePoolMessage};
use queue::QueueClient;
use serde_json::json;
use tokio::{signal, task::JoinSet};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    if cli.health_check {
        return run_health_check().await;
    }

    match cli.command {
        Some(Command::Tools { tool }) => run_tools(tool).await,
        _ => run_service().await,
    }
}

async fn run_tools(tool: Tool) -> Result<()> {
    info!("Executing tool: {:?}", tool);

    info!("Loading configuration from environment variables");
    let config = Config::try_from_env()?;

    match tool {
        Tool::QueueMatch { players } => {
            info!("Connecting to queue cluster...");
            let queue_client = QueueClient::new(&config.queue_cluster_url).await?;

            let match_id = format!("match_{}", chrono::Utc::now().timestamp());
            info!("Queuing match {} for players: {:?}", match_id, players);

            let result_handle = {
                let queue_client = queue_client.clone();
                let topic = queue_client.outgoing_topic().to_string();
                let match_id = match_id.clone();
                tokio::spawn(async move {
                    info!(
                        "Waiting for match result on topic '{}' with routing key '{}'",
                        topic, match_id
                    );
                    match queue_client.consume_one(&topic, &match_id).await {
                        Ok(data) => {
                            let message = String::from_utf8_lossy(&data);
                            info!("Received match result: {}", message);
                        }
                        Err(e) => {
                            error!("Failed to receive match result: {}", e);
                        }
                    }
                })
            };

            let message = json!({
                "match_id": match_id,
                "players": players
            });
            let data = serde_json::to_vec(&message)?;

            if let Err(e) = queue_client.publish_game_starting(&data).await {
                error!("Failed to queue match: {}", e);
            }

            // Wait for the result to be received
            result_handle.await?;
        }
    }

    Ok(())
}

async fn run_health_check() -> Result<()> {
    // Just ensures we can load config and connect to the queue.
    let config = Config::try_from_env()?;
    let queue_client = QueueClient::new(&config.queue_cluster_url).await?;
    queue_client.close().await?;
    info!("Health check successful.");
    Ok(())
}

async fn run_service() -> Result<()> {
    info!("It's-a Super Gametable!");

    info!("Loading configuration from environment variables");
    let config = Config::try_from_env()?;

    // --- Create shared clients ---
    info!("Connecting to queue cluster...");
    let queue_client = QueueClient::new(&config.queue_cluster_url).await?;

    // --- Create and wire up services ---
    let game_pool = GamePool::new(queue_client.clone());
    let game_pool_sender = game_pool.sender();

    let game_starting_handler = {
        let sender = game_pool_sender.clone();
        move |data: &[u8]| -> Result<()> {
            // TODO We need to back this with the spec crate
            let message: serde_json::Value = serde_json::from_slice(data)?;
            info!("Processing GameStarting message: {}", message);

            let match_id = message["match_id"].as_str().unwrap_or("").to_string();
            let players: Vec<String> = message["players"].as_array().map_or_else(Vec::new, |arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect()
            });

            if let Err(e) = sender.try_send(GamePoolMessage::StartGame { match_id, players }) {
                error!("Failed to send start game message: {}", e);
            }

            Ok(())
        }
    };

    let mut services = JoinSet::new();

    // Start the queue consumer
    services.spawn(async move {
        info!("Queue consumer starting.");
        if let Err(e) = queue_client
            .start_consuming(&config.incoming_queue_name, game_starting_handler)
            .await
        {
            error!("Queue consumer failed: {}", e);
        }
        info!("Queue consumer finished.");
    });

    // Start the game pool manager
    let _game_pool_handle = services.spawn(async move {
        info!("Game pool manager starting.");
        if let Err(e) = game_pool.run().await {
            error!("Game pool manager failed: {}", e);
        }
        info!("Game pool manager finished.");
    });

    // --- Run until shutdown ---
    info!("Super Gametable is running. Press Ctrl+C to shutdown.");
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Shutdown signal received.");
        },
        Some(res) = services.join_next() => {
            error!("A service task failed: {:?}", res);
        },
    }

    info!("Shutting down...");

    // Send shutdown message to game pool
    if let Err(e) = game_pool_sender.send(GamePoolMessage::Shutdown).await {
        error!("Failed to send shutdown message to game pool: {}", e);
    }

    // Abort all tasks in the JoinSet to signal them to shut down.
    // This will cause the loop below to resolve.
    services.abort_all();

    // Wait for all tasks to complete.
    while (services.join_next().await).is_some() {}

    info!("Super Gametable shut down gracefully.");
    Ok(())
}
