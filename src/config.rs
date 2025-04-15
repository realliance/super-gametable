//! Configuration management and parsing

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Listening port
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    8080
}

impl Config {
    pub fn try_from_env() -> Result<Self> {
        envy::from_env::<Config>()
            .map_err(|err| anyhow::anyhow!("Failed to load config from env: {}", err))
    }
}
