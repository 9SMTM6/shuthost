//! Configuration file watching and reloading utilities.
//!
//! This module provides functions for monitoring configuration files
//! for changes and automatically reloading them.

use std::{path::Path, sync::Arc};

use eyre::{Result, WrapErr};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc::unbounded_channel, watch};
use tracing::{error, info, warn};

use crate::config::ControllerConfig;

/// Handles the logic for reloading the configuration file and updating the application state.
///
/// This function is called when a file modification event is detected. It loads the new
/// configuration, checks for unsupported changes (like port or bind address), and sends the
/// new configuration to the application's state management channel.
///
/// # Arguments
///
/// * `path` - The path to the configuration file.
/// * `tx` - The sender part of a watch channel for broadcasting configuration updates.
/// * `initial_port` - The server port at application startup, used to detect changes.
/// * `initial_bind` - The server bind address at application startup, used to detect changes.
async fn process_config_change(
    path: &Path,
    tx: &watch::Sender<Arc<ControllerConfig>>,
    rx: &watch::Receiver<Arc<ControllerConfig>>,
) -> Result<()> {
    info!("Config file modified. Reloading...");
    let prev = rx.borrow().clone();
    let new_config = crate::config::load_coordinator_config(path)
        .await
        .wrap_err("Failed to reload config")?;
    // Determine what changed
    let server_changed = new_config.server != prev.server;
    let hosts_changed = new_config.hosts != prev.hosts;
    let clients_changed = new_config.clients != prev.clients;

    if server_changed {
        warn!(
            "Detected change to [server] config during runtime. Changes outside [hosts] and [clients] are not supported and will be ignored."
        );
    }

    if hosts_changed || clients_changed {
        // Only apply hosts/clients updates; keep prior server config
        let effective = ControllerConfig {
            server: prev.server.clone(),
            hosts: new_config.hosts,
            clients: new_config.clients,
        };
        tx.send(Arc::new(effective))
            .wrap_err("Failed to send updated config through watch channel")?;
        info!("Applied hosts/clients changes from config file.");
    } else if server_changed {
        // Only unsupported changes were made; nothing to apply
        info!("No applicable (hosts/clients) changes detected; ignoring unsupported updates.");
    } else {
        info!("No changes detected in config.");
    }
    Ok(())
}

/// Watches a config file for modifications and updates the provided channel on changes.
///
/// # Arguments
///
/// * `path` - Path to the config file to watch.
/// * `tx` - Watch channel sender to broadcast new config instances.
///
/// # Panics
///
/// Panics if the file watcher cannot be created.
pub async fn watch_config_file(path: std::path::PathBuf, tx: watch::Sender<Arc<ControllerConfig>>) {
    let (raw_tx, mut raw_rx) = unbounded_channel::<Event>();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res
                && raw_tx.send(event).is_err()
            {
                error!("Failed to send event to config watcher channel");
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .expect("Failed to watch config file");

    // Receiver used to read the current effective config for change comparisons
    let rx = tx.subscribe();

    while let Some(event) = raw_rx.recv().await {
        if matches!(event.kind, EventKind::Modify(_))
            && let Err(e) = process_config_change(&path, &tx, &rx).await
        {
            error!("Failed to process config change: {}", e);
        }
    }
}
