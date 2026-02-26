use std::collections::{HashMap, HashSet};

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use core::error::Error;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{Instrument as _, debug, error, info, warn};
use tungstenite::{Error as TError, error::ProtocolError as TPError};

use crate::{
    app::{
        db::{self, ClientStats, DbPool},
        state::{AppState, ConfigRx, HostStatus, HostStatusRx},
    },
    http::m2m::{LeaseMap, LeaseSource},
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

/// The set of lease sources for a single host
pub(crate) type LeaseSources = HashSet<LeaseSource>;

/// `host_name` => set of lease sources holding lease
pub(crate) type LeaseMapRaw = HashMap<String, LeaseSources>;

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    /// Gets sent on host status changes
    HostStatus(HostStatus),
    /// We watch for select config changes and update the `WebUI` to immediately
    /// reflect additions to hosts/clients.
    ConfigChanged {
        hosts: Vec<String>,
        clients: Vec<String>,
    },
    /// Send the entire state in the beginning to bootstrap the web client UI.
    Initial {
        hosts: Vec<String>,
        clients: Vec<String>,
        status: HostStatus,
        leases: LeaseMapRaw,
        client_stats: Option<HashMap<String, ClientStats>>,
        /// Broadcast port configured for the coordinator.
        broadcast_port: u16,
    },
    /// Gets sent on Lease status updates
    LeaseUpdate { host: String, leases: LeaseSources },
}

/// Gets called for every new web client and spins up an event loop
#[axum::debug_handler]
#[tracing::instrument(skip_all)]
pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(AppState {
        ws_tx,
        hoststatus_rx,
        config_rx,
        leases,
        db_pool,
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
    let hoststatus_rx = hoststatus_rx.clone();
    let config_rx = config_rx.clone();
    let current_leases = leases.clone();
    let db_pool_clone = db_pool.clone();

    // Log that we're returning an on_upgrade responder; the actual upgrade
    // happens asynchronously when the client completes the handshake.
    debug!("Registering WebSocket upgrade handler");

    ws.on_upgrade(async move |mut socket| {
        debug!("WebSocket upgrade completed; starting event loop");
        match send_startup_msg(
            &mut socket,
            hoststatus_rx,
            config_rx,
            current_leases,
            db_pool_clone.as_ref(),
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
            // Detect when the WebSocket is closed
            // Note that this doesn't seem to be catching all (or even any) closed connections.
            None = socket.recv() => {
                debug!("WebSocket connection closed");
                break;
            }
        }
    }
}

#[tracing::instrument(skip_all)]
async fn send_startup_msg(
    socket: &mut WebSocket,
    hoststatus_rx: HostStatusRx,
    config_rx: ConfigRx,
    current_leases: LeaseMap,
    db_pool: Option<&DbPool>,
) -> Result<(), axum::Error> {
    // Read freshest values from the receivers just before sending.
    let current_state = hoststatus_rx.borrow().clone();
    let config = config_rx.borrow().clone();

    let hosts = config.hosts.keys().cloned().collect();
    let clients = config.clients.keys().cloned().collect();
    let leases = { current_leases.lock().await.clone() };
    let client_stats = if let Some(pool) = db_pool {
        match db::get_all_client_stats(pool).await {
            Ok(stats) => Some(stats),
            Err(e) => {
                error!(%e, "Failed to get client stats");
                None
            }
        }
    } else {
        None
    };
    let initial_msg = WsMessage::Initial {
        hosts,
        clients,
        status: current_state.as_ref().clone(),
        leases,
        client_stats,
        broadcast_port: config.server.broadcast_port,
    };

    send_ws_message(socket, &initial_msg)
        .in_current_span()
        .await
}
