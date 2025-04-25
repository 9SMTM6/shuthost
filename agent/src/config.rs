use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub agent: AgentConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    pub port: u16,
    pub shutdown_command: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub shared_secret: String,
}

pub fn load_config(path: PathBuf) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
