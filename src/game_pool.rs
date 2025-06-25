//! Game pool management for handling multiple concurrent matches

use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::{spawn_blocking, JoinHandle};
use tracing::{error, info, warn};

use crate::controllers::GameController;
use crate::game::GameMatch;
use crate::queue::QueueClient;

/// Messages sent to the game pool for coordination
#[derive(Debug)]
pub enum GamePoolMessage {
    /// External command to start a new game
    StartGame {
        match_id: String,
        players: Vec<String>,
    },
    /// External command to clean up a finished game
    GameFinished { match_id: String },
    /// Internal notification that a game completed successfully
    GameComplete { match_id: String },
    /// Internal notification that a game ended in an error
    GameError { match_id: String, error: String },
    /// Command to shut down the entire game pool
    Shutdown,
}

/// Final status reported by a sync game runner
#[derive(Debug)]
pub enum GameStatus {
    Finished,
    Error(String),
}

/// Game pool manager that handles multiple concurrent games
pub struct GamePool {
    queue_client: QueueClient,
    message_tx: mpsc::Sender<GamePoolMessage>,
    message_rx: mpsc::Receiver<GamePoolMessage>,
}

impl GamePool {
    /// Create a new game pool
    pub fn new(queue_client: QueueClient) -> Self {
        let (message_tx, message_rx) = mpsc::channel(100);

        Self {
            queue_client,
            message_tx,
            message_rx,
        }
    }

    /// Get a sender for sending messages to the game pool
    pub fn sender(&self) -> mpsc::Sender<GamePoolMessage> {
        self.message_tx.clone()
    }

    /// Start the game pool manager
    pub async fn run(mut self) -> Result<()> {
        info!("Starting game pool manager");

        let mut active_games: HashMap<String, JoinHandle<()>> = HashMap::new();

        while let Some(message) = self.message_rx.recv().await {
            match message {
                GamePoolMessage::StartGame { match_id, players } => {
                    match self.start_game(match_id.clone(), players).await {
                        Ok(handle) => {
                            active_games.insert(match_id, handle);
                        }
                        Err(e) => {
                            error!("Failed to start game {}: {}", match_id, e);
                        }
                    }
                }
                GamePoolMessage::GameFinished { match_id } => {
                    info!(
                        "Received external notification to clean up game: {}",
                        match_id
                    );
                    if let Some(handle) = active_games.remove(&match_id) {
                        handle.abort();
                    }
                }
                GamePoolMessage::GameComplete { match_id } => {
                    info!("Game {} completed successfully", match_id);
                    if let Err(e) = self.handle_game_completion(&match_id).await {
                        error!("Error handling game completion for {}: {}", match_id, e);
                    }
                    active_games.remove(&match_id); // Task is done, just remove handle
                }
                GamePoolMessage::GameError { match_id, error } => {
                    error!("Game {} ended with an error: {}", match_id, error);
                    if let Err(e) = self.handle_game_completion(&match_id).await {
                        error!("Error handling game completion for {}: {}", match_id, e);
                    }
                    active_games.remove(&match_id);
                }
                GamePoolMessage::Shutdown => {
                    info!("Shutting down game pool");
                    for (match_id, handle) in active_games.drain() {
                        info!("Aborting game: {}", match_id);
                        handle.abort();
                    }
                    break;
                }
            }
        }

        info!("Game pool shut down");
        Ok(())
    }

    /// Start a new game in a background blocking task
    async fn start_game(&self, match_id: String, players: Vec<String>) -> Result<JoinHandle<()>> {
        info!(
            "Starting new game: {} with players: {:?}",
            match_id, players
        );

        let controllers: Vec<GameController> = (0..4)
            .map(|i| {
                let player_name = players
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| "AngryDiscardoBot".to_string());
                GameController::Embedded(player_name)
            })
            .collect();

        // Channel for the sync task to report its final status
        let (status_tx, mut status_rx) = mpsc::channel(1);

        // Spawn the entire game loop in a dedicated blocking thread
        // to avoid blocking the async runtime.
        let match_id_clone_blocking = match_id.clone();
        let handle = spawn_blocking(move || {
            Self::run_game_sync(match_id_clone_blocking, controllers, status_tx);
        });

        // Spawn an async task to bridge the result from the blocking
        // task back to the main game pool's message loop.
        let pool_sender = self.message_tx.clone();
        tokio::spawn(async move {
            if let Some(status) = status_rx.recv().await {
                let msg = match status {
                    GameStatus::Finished => GamePoolMessage::GameComplete {
                        match_id: match_id.clone(),
                    },
                    GameStatus::Error(e) => GamePoolMessage::GameError {
                        match_id: match_id.clone(),
                        error: e,
                    },
                };
                if let Err(e) = pool_sender.send(msg).await {
                    error!("Failed to send game result to pool for {}: {}", match_id, e);
                }
            }
        });

        Ok(handle)
    }

    /// Run game logic in a blocking thread
    fn run_game_sync(
        match_id: String,
        controllers: Vec<GameController>,
        status_tx: mpsc::Sender<GameStatus>,
    ) {
        info!("Sync game runner starting for match: {}", match_id);

        let mut game_match = match GameMatch::try_new(match_id.clone(), controllers) {
            Ok(game) => game,
            Err(e) => {
                error!("Failed to create game match {}: {}", match_id, e);
                let _ = status_tx.blocking_send(GameStatus::Error(e.to_string()));
                return;
            }
        };

        // Autonomous game loop that runs to completion
        let final_status = loop {
            match game_match.advance() {
                Ok(true) => {
                    // Game continues.
                    // Eventually advance will have a lot more to do with network waits
                    // where we probably wont need this sleep to prevent the CPU from
                    // getting pinned.
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Ok(false) => {
                    info!("Game {} finished.", match_id);
                    break GameStatus::Finished;
                }
                Err(e) => {
                    error!("Game {} failed to advance: {}", match_id, e);
                    break GameStatus::Error(e.to_string());
                }
            }
        };

        if let Err(e) = status_tx.blocking_send(final_status) {
            warn!(
                "Could not send final status for game {}: receiver dropped. {}",
                match_id, e
            );
        }
        info!("Sync game runner finished for match: {}", match_id);
    }

    /// Handle game completion (publish to queue, etc.)
    async fn handle_game_completion(&self, match_id: &str) -> Result<()> {
        info!("Publishing completion event for game: {}", match_id);
        let game_complete_data = Self::create_game_complete_message(match_id).await?;
        if let Err(e) = self
            .queue_client
            .publish_game_complete(match_id, &game_complete_data)
            .await
        {
            error!("Failed to publish game complete event: {}", e);
            return Err(e.into());
        }
        Ok(())
    }

    /// Create a GameComplete message
    async fn create_game_complete_message(match_id: &str) -> Result<Vec<u8>> {
        let message = json!({
            "match_id": match_id,
            "status": "completed"
        });
        Ok(serde_json::to_vec(&message)?)
    }
}
