use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{broadcast, watch};
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    pub ip: String,
    pub mac: String,
    pub port: u16,
    pub shared_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Client {
    pub shared_secret: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ControllerConfig {
    pub server: ServerConfig,
    pub nodes: HashMap<String, Node>,
    pub clients: HashMap<String, Client>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub port: u16,
    pub bind: String,
}

pub async fn load_coordinator_config<P: AsRef<Path>>(
    path: P,
) -> Result<ControllerConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let config: ControllerConfig = toml::from_str(&content)?;
    Ok(config)
}

pub async fn watch_config_file(
    path: std::path::PathBuf,
    tx: watch::Sender<Arc<ControllerConfig>>,
    ws_tx: broadcast::Sender<String>,
) {
    use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use tokio::sync::mpsc::unbounded_channel;

    let (raw_tx, mut raw_rx) = unbounded_channel::<Event>();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = raw_tx.send(event);
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .expect("Failed to watch config file");

    while let Some(event) = raw_rx.recv().await {
        if matches!(event.kind, EventKind::Modify(_)) {
            info!("Config file modified. Reloading...");
            match load_coordinator_config(&path).await {
                Ok(new_config) => {
                    let _ = tx.send(Arc::new(new_config));
                    info!("Config reloaded.");
                    let _ = ws_tx.send("config_updated".to_string());
                }
                Err(e) => {
                    error!("Failed to reload config: {}", e);
                }
            }
        }
    }
}
