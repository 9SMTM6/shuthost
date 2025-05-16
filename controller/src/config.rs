use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Host {
    pub ip: String,
    pub mac: String,
    pub port: u16,
    pub shared_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerConfig {
    pub server: ServerConfig,
    pub hosts: HashMap<String, Host>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub bind: String,
}

pub async fn load_controller_config<P: AsRef<Path>>(
    path: P,
) -> Result<ControllerConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let config: ControllerConfig = toml::from_str(&content)?;
    Ok(config)
}
