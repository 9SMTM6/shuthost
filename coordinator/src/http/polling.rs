//! Background polling tasks for the coordinator.

use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{broadcast, watch},
    time::{Instant, MissedTickBehavior, interval, timeout},
};
use tracing::{debug, info};

use shuthost_common::create_signed_message;

use crate::{
    config::{ControllerConfig, watch_config_file},
    websocket::WsMessage,
};

/// Poll a single host for its online status.
pub(crate) async fn poll_host_status(
    name: &str,
    ip: &str,
    port: u16,
    shared_secret: &secrecy::SecretString,
) -> bool {
    let addr = format!("{}:{}", ip, port);
    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(mut stream)) => {
            let signed_message = create_signed_message("status", shared_secret);
            if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                debug!("Failed to write to {}: {}", name, e);
                return false;
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
                    !resp.contains("ERROR")
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Poll a host until its state matches desired_state or timeout is reached. Updates global state.
///
/// # Errors
///
/// Returns an error if the polling times out or if there are issues with the host configuration.
pub(crate) async fn poll_until_host_state(
    host_name: &str,
    desired_state: bool,
    timeout_secs: u64,
    poll_interval_ms: u64,
    config_rx: &watch::Receiver<Arc<ControllerConfig>>,
    hoststatus_tx: &watch::Sender<Arc<HashMap<String, bool>>>,
) -> Result<(), String> {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let start = Instant::now();
    loop {
        let host = {
            let config = config_rx.borrow();
            match config.hosts.get(host_name) {
                Some(h) => h.clone(),
                None => return Err(format!("No configuration found for host '{}'.", host_name)),
            }
        };
        let poll_fut =
            poll_host_status(host_name, &host.ip, host.port, host.shared_secret.as_ref());
        let tick_fut = ticker.tick();
        let (is_online, _) = tokio::join!(poll_fut, tick_fut);
        // Update global state
        let mut status_map = hoststatus_tx.borrow().as_ref().clone();
        if status_map.get(host_name) != Some(&is_online) {
            status_map.insert(host_name.to_string(), is_online);
            if hoststatus_tx.send(Arc::new(status_map)).is_err() {
                debug!("Host status receiver dropped, stopping polling");
                return Err("Coordinator shutting down".to_string());
            }
        }
        if is_online == desired_state {
            return Ok(());
        }
        if start.elapsed().as_secs() >= timeout_secs {
            return Err(format!(
                "Timeout waiting for host '{host_name}' to become {}.",
                if desired_state { "online" } else { "offline" }
            ));
        }
    }
}

/// Start all background tasks for the HTTP server.
pub(crate) fn start_background_tasks(
    config_rx: &watch::Receiver<Arc<ControllerConfig>>,
    hoststatus_tx: &watch::Sender<Arc<HashMap<String, bool>>>,
    ws_tx: &broadcast::Sender<WsMessage>,
    config_tx: &watch::Sender<Arc<ControllerConfig>>,
    config_path: &Path,
) {
    // Start host status polling task
    {
        let config_rx = config_rx.clone();
        let hoststatus_tx = hoststatus_tx.clone();
        tokio::spawn(async move {
            poll_host_statuses(config_rx, hoststatus_tx).await;
        });
    }

    // Start config file watcher
    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(async move {
            watch_config_file(path, config_tx).await;
        });
    }

    // Forwards host status updates to the websocket client loops
    {
        let ws_tx = ws_tx.clone();
        let mut hoststatus_rx = hoststatus_tx.subscribe();
        tokio::spawn(async move {
            while hoststatus_rx.changed().await.is_ok() {
                let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                if ws_tx.send(msg).is_err() {
                    debug!("No Websocket Subscribers");
                }
            }
        });
    }

    // Forwards config changes to the websocket client loops
    {
        let ws_tx = ws_tx.clone();
        let mut config_rx = config_rx.clone();
        tokio::spawn(async move {
            while config_rx.changed().await.is_ok() {
                let config = config_rx.borrow();
                let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
                let clients = config.clients.keys().cloned().collect::<Vec<_>>();
                let msg = WsMessage::ConfigChanged { hosts, clients };
                if ws_tx.send(msg).is_err() {
                    debug!("No Websocket Subscribers");
                }
            }
        });
    }
}

/// Background task: periodically polls each host for status by attempting a TCP connection and HMAC ping.
async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    hoststatus_tx: watch::Sender<Arc<HashMap<String, bool>>>,
) {
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
                let is_online = poll_host_status(&name, &ip, port, shared_secret.as_ref()).await;
                debug!("Polled {} at {}:{} - online: {}", name, ip, port, is_online);
                (name, is_online)
            }
        });

        let results = futures::future::join_all(futures).await;
        let status_map: HashMap<_, _> = results.into_iter().collect();

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
