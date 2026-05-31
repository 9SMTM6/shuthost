use alloc::sync::Arc;
use core::error::Error;
use std::collections::HashMap;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::broadcast;
use tracing::{Instrument as _, debug, error, info, warn};
use tungstenite::{Error as TError, error::ProtocolError as TPError};

use crate::app::{
    AppState, ConfigRx, DbPool, HostState, HostStatus, HostStatusRx, LeaseMap, LeaseSources,
    LeaseStore, OperationFailureMap,
    db::{self, ClientStats, HostStats},
};

/// Walk the error source chain and return true if any source is an error about the websocket being closed.
fn is_websocket_closed(err: &axum::Error) -> bool {
    let mut current: &(dyn Error + 'static) = err;
    loop {
        // Try downcasting the current error trait object to a concrete tungstenite::Error
        if matches!(
            current.downcast_ref::<TError>(),
            Some(TError::AlreadyClosed | TError::Protocol(TPError::SendAfterClosing))
        ) {
            return true;
        }

        match current.source() {
            Some(src) => current = src,
            None => break,
        }
    }
    false
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", tag = "status", content = "payload")]
pub enum DbDataState {
    Disabled,
    #[serde(rename_all = "camelCase")]
    Available {
        client_stats: HashMap<String, ClientStats>,
        host_stats: HashMap<String, HostStats>,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    /// Gets sent on host status changes
    HostStatus(HostStatus),
    /// Gets sent when client stats are updated.
    ClientStats(HashMap<String, ClientStats>),
    /// Gets sent when host stats are updated.
    HostStats { host: String, stats: HostStats },
    /// We watch for select config changes and update the `WebUI` to immediately
    /// reflect additions to hosts/clients.
    ConfigChanged {
        hosts: Vec<String>,
        clients: Vec<String>,
    },
    /// Send the entire state in the beginning to bootstrap the web client UI.
    #[serde(rename_all = "camelCase")]
    Initial {
        hosts: Vec<String>,
        clients: Vec<String>,
        status_map: HostStatus,
        lease_map: LeaseMap,
        db_data: DbDataState,
        operation_failures: OperationFailureMap,
    },
    /// Gets sent on Lease status updates
    LeaseUpdate { host: String, leases: LeaseSources },
    /// Gets sent when a host's last control operation failure state changes.
    OperationFailed(OperationFailureMap),
}

/// Gets called for every new web client and spins up an event loop
#[axum::debug_handler]
#[tracing::instrument(skip_all)]
pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(AppState {
        ws_tx,
        host_actor,
        config_rx,
        leases,
        db_pool,
        operation_failures,
        ..
    }): State<AppState>,
) -> impl IntoResponse {
    // Log incoming headers so we can verify whether the Upgrade/Connection
    // and other WebSocket-related headers reach the backend (useful when
    // Traefik or another proxy is in front).
    debug!(?headers, "Incoming WebSocket upgrade headers");

    // Defer reading current state until inside the startup sender so we get the
    // freshest values at the moment of sending. Clone the receivers/leases to
    // move into the upgrade task.
    let config_rx = config_rx.clone();
    let current_leases = leases.clone();
    let db_pool_clone = db_pool.clone();

    // Log that we're returning an on_upgrade responder; the actual upgrade
    // happens asynchronously when the client completes the handshake.
    debug!("Registering WebSocket upgrade handler");

    let op_failures_snapshot = operation_failures.borrow().clone();

    ws.on_upgrade(async move |mut socket| {
        debug!("WebSocket upgrade completed; starting event loop");
        match send_startup_msg(
            &mut socket,
            host_actor.subscribe_status(),
            config_rx,
            current_leases,
            db_pool_clone.as_ref(),
            op_failures_snapshot,
        )
        .await
        {
            Ok(()) => {}
            Err(e) => {
                warn!("Failed to send initial state: {}", e);
                return;
            }
        }
        start_webui_ws_loop(socket, ws_tx.subscribe()).await;
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn send_ws_message(socket: &mut WebSocket, msg: &WsMessage) -> Result<(), axum::Error> {
    match serde_json::to_string(msg) {
        Ok(json) => socket.send(Message::Text(json.into())).await,
        Err(e) => {
            warn!(%e, "Failed to serialize websocket message");
            Err(axum::Error::new(e))
        }
    }
}

/// We start one event loop per client
#[tracing::instrument(level = "debug", skip_all)]
async fn start_webui_ws_loop(mut socket: WebSocket, mut rx: broadcast::Receiver<WsMessage>) {
    // Handle broadcast messages
    loop {
        tokio::select! {
            // Receive messages from the broadcast channel
            msg = rx.recv() => {
                match msg {
                    Ok(msg) => {
                        if let Err(e) = send_ws_message(&mut socket, &msg).await {
                            let closed = is_websocket_closed(&e);
                            if closed {
                                debug!("WebSocket connection closed");
                            } else {
                                warn!("Failed to send message, closing connection: {}", e);
                            }
                            break;
                        }
                    }
                    Err(_) => {
                        info!("Broadcast channel closed, stopping WebSocket handler");
                        break;
                    }
                }
            }
                // Handle incoming messages from the client, including control pings.
                incoming = socket.recv() => {
                    match incoming {
                        Some(Ok(msg)) => {
                            match msg {
                                Message::Text(t) => {
                                    // Try to parse as JSON control frame { type: 'ping' }
                                    if let Ok(json) = serde_json::from_str::<JsonValue>(&t)
                                        && let Some(tp) = json.get("type").and_then(|v| v.as_str())
                                            && tp == "ping" {
                                                // Reply with an app-level pong
                                                let pong = serde_json::json!({"type": "pong"}).to_string();
                                                if let Err(e) = socket.send(Message::Text(pong.into())).await {
                                                    warn!(%e, "Failed to send pong");
                                                    break;
                                                }
                                            }
                                    // Not a control message — ignore here (server only expects to send broadcasts)
                                }
                                Message::Ping(payload) => {
                                    // Respond at protocol level
                                    if let Err(e) = socket.send(Message::Pong(payload)).await {
                                        warn!(%e, "Failed to send protocol Pong");
                                        break;
                                    }
                                }
                                Message::Pong(_) | Message::Binary(_) => {
                                    // Pong: client answered a server ping — nothing to do on server side
                                    // Binary: we don't expect to receive any binary messages
                                }
                                Message::Close(_) => {
                                    debug!("WebSocket connection closed by client");
                                    break;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            warn!(%e, "WebSocket recv error");
                            break;
                        }
                        None => {
                            debug!("WebSocket connection closed");
                            break;
                        }
                    }
                }
        }
    }
}

#[tracing::instrument(skip_all)]
async fn send_startup_msg(
    socket: &mut WebSocket,
    hoststatus_rx: HostStatusRx,
    config_rx: ConfigRx,
    current_leases: Arc<LeaseStore>,
    db_pool: Option<&DbPool>,
    operation_failures: Arc<OperationFailureMap>,
) -> Result<(), axum::Error> {
    // Read freshest values from the receivers just before sending.
    let current_state = hoststatus_rx.borrow().clone();
    let config = config_rx.borrow().clone();
    let mut status_map = current_state.as_ref().clone();
    for host in config.hosts.keys() {
        status_map.entry(host.clone()).or_insert(HostState::Offline);
    }

    let hosts = config.hosts.keys().cloned().collect();
    let clients = config.clients.keys().cloned().collect();
    let leases = (*current_leases.snapshot()).clone();
    let db_data = if let Some(pool) = db_pool {
        let client_stats = db::get_all_client_stats(pool).await;
        let host_stats = db::get_all_host_stats(pool).await;

        match (client_stats, host_stats) {
            (Ok(client_stats), Ok(mut host_stats)) => {
                for (name, &state) in current_state.iter() {
                    if state == HostState::Online {
                        host_stats.entry(name.clone()).or_default().is_online = true;
                    }
                }
                DbDataState::Available {
                    client_stats,
                    host_stats,
                }
            }
            (client_err, host_err) => {
                let err = client_err
                    .err()
                    .or_else(|| host_err.err())
                    .expect("one of the DB futures must have failed");
                error!(%err, "Failed to load db startup stats");
                DbDataState::Disabled
            }
        }
    } else {
        DbDataState::Disabled
    };
    let initial_msg = WsMessage::Initial {
        hosts,
        clients,
        status_map,
        lease_map: leases,
        db_data,
        operation_failures: operation_failures.as_ref().clone(),
    };

    send_ws_message(socket, &initial_msg)
        .in_current_span()
        .await
}
