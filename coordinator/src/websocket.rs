use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast, watch};
use tracing::{info, warn, error};
use tungstenite::{Error as TError, error::ProtocolError as TPError};

/// Walk the error source chain and return true if any source is an error about the websocket being closed.
fn is_websocket_closed(err: &axum::Error) -> bool {
    let mut current: &(dyn std::error::Error + 'static) = err;
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

use crate::{
    config::ControllerConfig,
    db::{self, ClientStats},
    http::{AppState, m2m::LeaseSource},
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    /// Gets sent on host status changes
    HostStatus(HashMap<String, bool>),
    /// We watch for select config changes and update the WebUI to immediately reflect additions to hosts or clients
    ConfigChanged {
        hosts: Vec<String>,
        clients: Vec<String>,
    },
    /// Send the entire state in the beginning to bootstrap the web client UI.
    Initial {
        hosts: Vec<String>,
        clients: Vec<String>,
        status: HashMap<String, bool>,
        leases: HashMap<String, HashSet<LeaseSource>>,
        client_stats: HashMap<String, ClientStats>,
    },
    /// Gets sent on Lease status updates
    LeaseUpdate {
        host: String,
        leases: HashSet<LeaseSource>,
    },
}

/// Gets called for every new web client and spins up an event loop
#[axum::debug_handler]
pub async fn ws_handler(
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
    info!(?headers, "Incoming WebSocket upgrade headers");

    // Defer reading current state until inside the startup sender so we get the
    // freshest values at the moment of sending. Clone the receivers/leases to
    // move into the upgrade task.
    let hoststatus_rx = hoststatus_rx.clone();
    let config_rx = config_rx.clone();
    let current_leases = leases.clone();
    let db_pool_clone = db_pool.clone();

    // Log that we're returning an on_upgrade responder; the actual upgrade
    // happens asynchronously when the client completes the handshake.
    info!("Registering WebSocket upgrade handler");

    ws.on_upgrade(async move |mut socket| {
        info!("WebSocket upgrade completed; starting event loop");
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
        };
        start_webui_ws_loop(socket, ws_tx.subscribe()).await;
    })
}

async fn send_ws_message(socket: &mut WebSocket, msg: &WsMessage) -> Result<(), axum::Error> {
    match serde_json::to_string(msg) {
        Ok(json) => socket.send(Message::Text(json.into())).await,
        Err(e) => {
            warn!("Failed to serialize websocket message: {}", e);
            Err(axum::Error::new(e))
        }
    }
}

/// We start one event loop per client
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
                                info!("WebSocket connection closed");
                            } else {
                                warn!("Failed to send message, closing connection: {}", e);
                            }
                            break;
                        }
                    }
                    Err(_) => {
                        warn!("Broadcast channel closed, stopping WebSocket handler");
                        break;
                    }
                }
            }
            // Detect when the WebSocket is closed
            // Note that this doesn't seem to be catching all (or even any) closed connections.
            None = socket.recv() => {
                info!("WebSocket connection closed");
                break;
            }
        }
    }
}

async fn send_startup_msg(
    socket: &mut WebSocket,
    hoststatus_rx: watch::Receiver<Arc<HashMap<String, bool>>>,
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    current_leases: Arc<Mutex<HashMap<String, HashSet<LeaseSource>>>>,
    db_pool: Option<&crate::db::DbPool>,
) -> Result<(), axum::Error> {
    // Read freshest values from the receivers just before sending.
    let current_state = hoststatus_rx.borrow().clone();
    let config = config_rx.borrow().clone();

    let hosts = config.hosts.keys().cloned().collect();
    let clients = config.clients.keys().cloned().collect();
    let leases = { current_leases.lock().await.clone() };
    let client_stats = if let Some(pool) = db_pool {
        match db::get_all_client_stats(pool).await {
            Ok(stats) => stats,
            Err(e) => {
                error!("Failed to get client stats: {}", e);
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };
    let initial_msg = WsMessage::Initial {
        hosts,
        clients,
        status: current_state.as_ref().clone(),
        leases,
        client_stats,
    };

    send_ws_message(socket, &initial_msg).await
}
