use serde::Deserialize;
use std::collections::HashMap;
use tokio::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Host {
    pub ip: String,
    pub mac: String,
    pub port: u16,
    pub shared_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct ControllerConfig {
    pub hosts: HashMap<String, Host>,
}

pub async fn load_controller_config<P: AsRef<Path>>(path: P) -> Result<ControllerConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let config: ControllerConfig = toml::from_str(&content)?;
    Ok(config)
}
