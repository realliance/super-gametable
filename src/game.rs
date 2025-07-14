use anyhow::Result;
use libmahjong_rs::{
    ffi::{error::MahjongFFIError, gamestate::GameState},
    observe::{ObservedGameState, StateFunctionType},
    settings::GameSettings,
};
use rand::Rng;
use tracing::info;

use crate::controllers::GameController;

/// Represents a single game match to execute
/// Libmahjong matches are an iterated on state machine,
/// which has hooks to it's game controllers.
/// Those game controllers might be included with the engine,
/// or registered externally and managed as a part of this Game object.
///
/// Game matches should be iterated to completion
pub struct GameMatch {
    state: Option<GameState>,
    match_id: String,
}

impl GameMatch {
    /// Try to create a new game match
    pub fn try_new(match_id: String, controllers: Vec<GameController>) -> Result<Self> {
        let controller_strings: Vec<String> = controllers.iter().map(|c| c.to_string()).collect();
        let seat_controllers: [String; 4] = controller_strings
            .try_into()
            .map_err(|_| anyhow::anyhow!("Expected exactly 4 controllers"))?;

        let settings = GameSettings {
            seat_controllers,
            seed: rand::thread_rng().gen(),
        };

        Ok(Self {
            state: Some(GameState::new(settings)?),
            match_id,
        })
    }

    /// Advance the game state
    pub fn advance(&mut self) -> Result<bool> {
        if let Some(current_state) = self.state.take() {
            match current_state.advance() {
                Ok(new_state) => {
                    self.state = Some(new_state);
                    let observed = self
                        .observe_state()
                        .ok_or(MahjongFFIError::GameStateConsumed)?;
                    if observed.current_state() == StateFunctionType::GameEnd {
                        info!("Game {} finished: {:?}", self.match_id, observed);
                        return Ok(false); // Game is done
                    }

                    Ok(true) // Game continues
                }
                Err(MahjongFFIError::GameEnded) => {
                    // Game is finished, state remains None
                    Ok(false) // Game is done
                }
                Err(e) => {
                    // Propagate other errors
                    Err(e.into())
                }
            }
        } else {
            Err(anyhow::anyhow!("Attempted to advance a finished game"))
        }
    }

    /// Observe the current game state
    pub fn observe_state(&self) -> Option<ObservedGameState> {
        self.state.as_ref().and_then(|s| s.observe())
    }
}
