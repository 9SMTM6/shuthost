use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
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

pub fn load_controller_config<P: AsRef<Path>>(path: P) -> Result<ControllerConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: ControllerConfig = toml::from_str(&content)?;
    Ok(config)
}
