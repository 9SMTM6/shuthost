//! Configuration management for the coordinator: loading and watching the TOML config file.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{mpsc::unbounded_channel, watch};
use tracing::{error, info};


/// Represents a configured host entry with network and security parameters.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Host {
    /// IP address of the host agent.
    pub ip: String,
    /// MAC address of the host agent's network interface.
    pub mac: String,
    /// TCP port the host agent listens on.
    pub port: u16,
    /// Shared secret for HMAC authentication.
    pub shared_secret: String,
}

/// Configuration for a client with its shared secret.
#[derive(Debug, Serialize, Deserialize)]
pub struct Client {
    /// Shared secret used for authenticating callbacks.
    pub shared_secret: String,
}

/// Root config structure for the coordinator, including server settings, hosts, and clients.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ControllerConfig {
    /// HTTP server binding configuration.
    pub server: ServerConfig,
    /// Map of host identifiers to host configurations.
    pub hosts: HashMap<String, Host>,
    /// Map of client identifiers to client configurations.
    pub clients: HashMap<String, Client>,
}

/// HTTP server binding configuration section.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// TCP port for the web control service.
    pub port: u16,
    /// Bind address for the HTTP listener.
    pub bind: String,
}

/// Reads and parses the coordinator config from a TOML file.
///
/// # Arguments
///
/// * `path` - File path to the TOML configuration file.
///
/// # Returns
///
/// A `ControllerConfig` on success, or an error boxed trait object.
pub async fn load_coordinator_config<P: AsRef<Path>>(
    path: P,
) -> Result<ControllerConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let config: ControllerConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Watches a config file for modifications and updates the provided channel on changes.
///
/// # Arguments
///
/// * `path` - Path to the config file to watch.
/// * `tx` - Watch channel sender to broadcast new config instances.
pub async fn watch_config_file(path: std::path::PathBuf, tx: watch::Sender<Arc<ControllerConfig>>) {
    let (raw_tx, mut raw_rx) = unbounded_channel::<Event>();
    let initial_config = tx.borrow().clone();
    let initial_port = initial_config.server.port;
    let initial_bind = initial_config.server.bind.clone();

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
                    if new_config.server.port != initial_port {
                        error!(
                            "Port change detected in config file. Changing ports while the server is running is not supported. Server will continue to run on port {}",
                            initial_port
                        );
                    }
                    if new_config.server.bind != initial_bind {
                        error!(
                            "Bind address change detected in config file. Changing bind address while the server is running is not supported. Server will continue to run on {}",
                            initial_bind
                        );
                    }
                    let _ = tx.send(Arc::new(new_config));
                    info!("Config reloaded.");
                }
                Err(e) => {
                    error!("Failed to reload config: {}", e);
                }
            }
        }
    }
}
