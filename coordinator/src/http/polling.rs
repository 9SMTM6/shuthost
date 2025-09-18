//! Background polling tasks for the coordinator.

use std::path::Path;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, watch};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::config::{ControllerConfig, watch_config_file};
use crate::websocket::WsMessage;
use shuthost_common::create_signed_message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Start all background tasks for the HTTP server.
pub fn start_background_tasks(
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

    // Start WebSocket host status broadcaster
    {
        let ws_tx = ws_tx.clone();
        let mut hoststatus_rx = hoststatus_tx.subscribe();
        tokio::spawn(async move {
            while hoststatus_rx.changed().await.is_ok() {
                let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                if ws_tx.send(msg).is_err() {
                    warn!("Failed to send WebSocket message");
                }
            }
        });
    }

    // Start WebSocket config change broadcaster
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
                    warn!("Failed to send WebSocket message");
                }
            }
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
}

/// Background task: periodically polls each host for status by attempting a TCP connection and HMAC ping.
async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    hoststatus_tx: watch::Sender<Arc<HashMap<String, bool>>>,
) {
    loop {
        let config = config_rx.borrow().clone();

        let futures = config.hosts.iter().map(|(name, host)| {
            let addr = format!("{}:{}", host.ip, host.port);
            let name = name.clone();
            let shared_secret = host.shared_secret.clone();
            async move {
                let is_online =
                    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
                        Ok(Ok(mut stream)) => {
                            let signed_message = create_signed_message("status", &shared_secret);
                            // Send message
                            if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                                debug!("Failed to write to {}: {}", name, e);
                                return (name, false);
                            }
                            // Read response (optional, but let's check for a valid reply)
                            let mut buf = vec![0u8; 256];
                            match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                                Ok(Ok(n)) if n > 0 => {
                                    let Some(data) = buf.get(..n) else {
                                        unreachable!("Read data size should always be valid, as its >= buffer size");
                                    };
                                    let resp = String::from_utf8_lossy(data);
                                    // Accept any non-error response as online
                                    !resp.contains("ERROR")
                                }
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                debug!("Polled {} at {} - online: {}", name, addr, is_online);
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
            hoststatus_tx.send(Arc::new(status_map)).unwrap();
        } else {
            debug!("No change in host status");
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
