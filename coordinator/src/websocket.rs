use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{Mutex, broadcast};
use tracing::{info, warn};

use crate::{config::ControllerConfig, http::AppState, routes::LeaseSource};

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
        leases: HashMap<String, Vec<LeaseSource>>,
    },
    /// Gets sent on Lease status updates
    LeaseUpdate {
        host: String,
        leases: Vec<LeaseSource>,
    },
}

/// Gets called for every new web client and spins up an event loop
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(AppState {
        ws_tx,
        hoststatus_rx,
        config_rx,
        leases,
        ..
    }): State<AppState>,
) -> impl IntoResponse {
    let current_state = hoststatus_rx.borrow().clone();
    let current_config = config_rx.borrow().clone();
    let current_leases = leases.clone();
    ws.on_upgrade(move |socket| {
        start_webui_ws_loop(
            socket,
            ws_tx.subscribe(),
            current_state,
            current_config,
            current_leases,
        )
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
async fn start_webui_ws_loop(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<WsMessage>,
    current_state: Arc<HashMap<String, bool>>,
    config: Arc<ControllerConfig>,
    current_leases: Arc<Mutex<HashMap<String, HashSet<LeaseSource>>>>,
) {
    tokio::spawn(async move {
        // Send initial combined state
        let hosts = config.hosts.keys().cloned().collect();
        let clients = config.clients.keys().cloned().collect();
        let leases_map = {
            current_leases
                .lock()
                .await
                .iter()
                .map(|(host, sources)| (host.clone(), sources.iter().cloned().collect()))
                .collect::<HashMap<_, _>>()
        };
        let initial_msg = WsMessage::Initial {
            hosts,
            clients,
            status: current_state.as_ref().clone(),
            leases: leases_map.clone(), // Pass the lease data
        };

        if let Err(e) = send_ws_message(&mut socket, &initial_msg).await {
            warn!("Failed to send initial state: {}", e);
            return;
        }

        // Handle broadcast messages
        loop {
            tokio::select! {
                // Receive messages from the broadcast channel
                msg = rx.recv() => {
                    match msg {
                        Ok(msg) => {
                            if let Err(e) = send_ws_message(&mut socket, &msg).await {
                                warn!("Failed to send message, closing connection: {}", e);
                                break;
                            }
                        }
                        Err(_) => {
                            warn!("Broadcast channel closed, stopping WebSocket handler");
                            break;
                        }
                    }
                }
                // TODO: isnt properly working.
                // Detect when the WebSocket is closed
                None = socket.recv() => {
                    info!("WebSocket connection closed");
                    break;
                }
            }
        }
    });
}
