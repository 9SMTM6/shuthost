//! Background polling tasks for the coordinator.

use alloc::sync::Arc;
use core::time::Duration;
use std::{collections::HashMap, path::Path};

use futures::future;
use thiserror::Error as ThisError;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::{Instant, MissedTickBehavior, interval, timeout},
};
use tracing::{Instrument as _, debug, info};

use shuthost_common::create_signed_message;

use super::state::{ConfigRx, ConfigTx, HostState, HostStatusTx, WsTx};
use crate::{app::config_watcher::watch_config_file, websocket::WsMessage};

/// Poll a single host for its online status.
async fn poll_host_status(
    name: &str,
    ip: &str,
    port: u16,
    shared_secret: &secrecy::SecretString,
) -> HostState {
    let addr = format!("{ip}:{port}");
    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(mut stream)) => {
            let signed_message = create_signed_message("status", shared_secret);
            if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                debug!("Failed to write to {}: {}", name, e);
                return HostState::Offline;
            }
            let mut buf = vec![0u8; 256];
            match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    let Some(data) = buf.get(..n) else {
                        unreachable!(
                            "Read data size should always be valid, as its >= buffer size"
                        );
                    };
                    let resp = String::from_utf8_lossy(data);
                    // Accept any non-error response as online
                    if resp.contains("ERROR") {
                        HostState::Offline
                    } else {
                        HostState::Online
                    }
                }
                _ => HostState::Offline,
            }
        }
        _ => HostState::Offline,
    }
}

/// Poll a host until its state matches `desired_state` or timeout is reached. Updates global state.
///
/// # Errors
///
/// Returns an error if the polling times out or if there are issues with the host configuration.
#[derive(Debug, ThisError)]
pub(super) enum PollError {
    #[error("No configuration found for host")]
    NotFound,
    #[error("Timeout waiting for host '{host_name}' to become {desired_state:?}")]
    Timeout {
        host_name: String,
        desired_state: HostState,
    },
    #[error("Coordinator shutting down")]
    CoordinatorShuttingDown,
}

pub(super) async fn poll_until_host_state(
    host_name: &str,
    desired_state: HostState,
    timeout_secs: u64,
    poll_interval_ms: u64,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), PollError> {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let start = Instant::now();
    loop {
        let host = {
            let config = config_rx.borrow();
            match config.hosts.get(host_name) {
                Some(h) => h.clone(),
                None => return Err(PollError::NotFound),
            }
        };
        let poll_fut =
            poll_host_status(host_name, &host.ip, host.port, host.shared_secret.as_ref());
        let tick_fut = ticker.tick();
        let (current_state, _) = tokio::join!(poll_fut, tick_fut);
        // Update global state
        let mut status_map = hoststatus_tx.borrow().as_ref().clone();
        if status_map.get(host_name) != Some(&current_state) {
            status_map.insert(host_name.to_string(), current_state);
            if hoststatus_tx.send(Arc::new(status_map)).is_err() {
                debug!("Host status receiver dropped, stopping polling");
                return Err(PollError::CoordinatorShuttingDown);
            }
        }
        if current_state == desired_state {
            return Ok(());
        }
        if start.elapsed().as_secs() >= timeout_secs {
            return Err(PollError::Timeout {
                host_name: host_name.to_string(),
                desired_state,
            });
        }
    }
}

/// Start all background tasks for the HTTP server.
pub(super) fn start_background_tasks(
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
    ws_tx: &WsTx,
    config_tx: &ConfigTx,
    config_path: &Path,
) {
    // Start host status polling task
    {
        let config_rx = config_rx.clone();
        let hoststatus_tx = hoststatus_tx.clone();
        tokio::spawn(
            async move {
                poll_host_statuses(config_rx, hoststatus_tx).await;
            }
            .in_current_span(),
        );
    }

    // Start config file watcher
    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(
            async move {
                watch_config_file(path, config_tx).await;
            }
            .in_current_span(),
        );
    }

    // Forwards host status updates to the websocket client loops
    {
        let ws_tx = ws_tx.clone();
        let mut hoststatus_rx = hoststatus_tx.subscribe();
        tokio::spawn(
            async move {
                while hoststatus_rx.changed().await.is_ok() {
                    let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                    if ws_tx.send(msg).is_err() {
                        debug!("No Websocket Subscribers");
                    }
                }
            }
            .in_current_span(),
        );
    }

    // Forwards config changes to the websocket client loops
    {
        let ws_tx = ws_tx.clone();
        let mut config_rx = config_rx.clone();
        tokio::spawn(
            async move {
                while config_rx.changed().await.is_ok() {
                    let config = config_rx.borrow();
                    let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
                    let clients = config.clients.keys().cloned().collect::<Vec<_>>();
                    let msg = WsMessage::ConfigChanged { hosts, clients };
                    if ws_tx.send(msg).is_err() {
                        debug!("No Websocket Subscribers");
                    }
                }
            }
            .in_current_span(),
        );
    }
}

/// Background task: periodically polls each host for status by attempting a TCP connection and HMAC ping.
async fn poll_host_statuses(config_rx: ConfigRx, hoststatus_tx: HostStatusTx) {
    let poll_interval = Duration::from_secs(2);
    let mut ticker = interval(poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let config = config_rx.borrow().clone();

        let futures = config.hosts.iter().map(|(name, host)| {
            let name = name.clone();
            let ip = host.ip.clone();
            let port = host.port;
            let shared_secret = host.shared_secret.clone();
            async move {
                let state = poll_host_status(&name, &ip, port, shared_secret.as_ref()).await;
                debug!("Polled {} at {}:{} - state: {:?}", name, ip, port, state);
                (name, state)
            }
        });

        let results = future::join_all(futures).await;
        let status_map: HashMap<String, HostState> = results.into_iter().collect();

        let is_new = {
            let old_status_map = hoststatus_tx.borrow();
            let old_status_map = old_status_map.as_ref();
            status_map != *old_status_map
        };
        if is_new {
            info!("Host status changed: {:?}", status_map);
            if hoststatus_tx.send(Arc::new(status_map)).is_err() {
                debug!("Host status receiver dropped, stopping polling");
                break;
            }
        } else {
            debug!("No change in host status");
        }

        ticker.tick().await;
    }
}
