//! Configuration file watching and reloading utilities.
//!
//! This module provides functions for monitoring configuration files
//! for changes and automatically reloading them.

use alloc::sync::Arc;
use std::{
    fs,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr as _};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::sync::mpsc::unbounded_channel;
use tracing::{error, info, warn};

use super::state::{ConfigRx, ConfigTx};
use crate::config::{self, ControllerConfig};

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
/// * `rx` - The receiver part of a watch channel for reading the current configuration state.
async fn process_config_change(path: &Path, tx: &ConfigTx, rx: &ConfigRx) -> Result<()> {
    info!("Config file modified. Reloading...");
    let prev = rx.borrow().clone();
    let new_config = config::load(path)
        .await
        .wrap_err(format!("Failed to reload config at: {}", path.display()))?;
    let effective = ControllerConfig {
        hosts: new_config.hosts.clone(),
        clients: new_config.clients.clone(),
        ..prev.as_ref().clone()
    };
    // Determine what changed
    let uneffective_change = effective != new_config;
    let hosts_changed = new_config.hosts != prev.hosts;
    let clients_changed = new_config.clients != prev.clients;

    if uneffective_change {
        warn!(
            "Detected change outside of [hosts] and [clients] during runtime. Such changes are unsupported and will be ignored."
        );
    }

    if hosts_changed || clients_changed {
        // Only apply hosts/clients updates; keep prior server config
        tx.send(Arc::new(effective))
            .wrap_err("Failed to send updated config through watch channel")?;
        info!("Applied hosts/clients changes from config file.");
    } else if uneffective_change {
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
/// Panics if the file watcher cannot be created or if the config file doesnt have a parent directory.
pub(super) async fn watch_config_file(path: PathBuf, tx: ConfigTx) {
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

    let dir = path
        .parent()
        .expect("Config file must have a parent directory");
    watcher
        .watch(dir, RecursiveMode::NonRecursive)
        .expect("Failed to watch config directory");

    // Receiver used to read the current effective config for change comparisons
    let rx = tx.subscribe();

    // Get the filename to match against, as a fallback for path comparison issues
    let config_filename = path.file_name().expect("Config file must have a filename");

    while let Some(event) = raw_rx.recv().await {
        if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
            // Check if any of the event paths match our config file
            // We check both exact path match and filename match (for atomic writes)
            let matches_config = event.paths.iter().any(|event_path| {
                // Try exact match first
                if event_path == &path {
                    return true;
                }
                // Try canonicalized comparison (handles path format differences)
                if let (Ok(canonical_event), Ok(canonical_config)) =
                    (fs::canonicalize(event_path), fs::canonicalize(&path))
                    && canonical_event == canonical_config
                {
                    return true;
                }
                // Fallback to filename match (handles atomic writes where temp files are involved)
                if let Some(event_filename) = event_path.file_name()
                    && event_filename == config_filename
                {
                    return true;
                }
                false
            });

            if matches_config && let Err(e) = process_config_change(&path, &tx, &rx).await {
                error!(?e, "Failed to process config change");
                break;
            }
        }
    }
}
