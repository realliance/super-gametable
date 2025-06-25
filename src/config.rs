//! Configuration management and parsing

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub queue_cluster_url: String,
    pub incoming_queue_name: String,
}

impl Config {
    pub fn try_from_env() -> Result<Self> {
        envy::from_env::<Config>()
            .map_err(|err| anyhow::anyhow!("Failed to load config from env: {}", err))
    }
}
