use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::broadcast;
use tracing::warn;
use serde::{Serialize, Deserialize};

use crate::{config::ControllerConfig, http::AppState};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    HostStatus(HashMap<String, bool>),
    UpdateNodes(Vec<String>),
    Initial {
        nodes: Vec<String>,
        status: HashMap<String, bool>,
    },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(AppState {
        ws_tx,
        hoststatus_rx,
        config_rx,
        ..
    }): State<AppState>,
) -> impl IntoResponse {
    let current_state = hoststatus_rx.borrow().clone();
    let current_config = config_rx.borrow().clone();
    ws.on_upgrade(move |socket| handle_socket(
        socket,
        ws_tx.subscribe(),
        current_state,
        current_config
    ))
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

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<WsMessage>,
    current_state: Arc<HashMap<String, bool>>,
    config: Arc<ControllerConfig>,
) {
    tokio::spawn(async move {
        // Send initial combined state
        let nodes = config.nodes.keys().cloned().collect();
        let initial_msg = WsMessage::Initial {
            nodes,
            status: current_state.as_ref().clone(),
        };
        
        if let Err(e) = send_ws_message(&mut socket, &initial_msg).await {
            warn!("Failed to send initial state: {}", e);
            return;
        }

        // Handle broadcast messages
        while let Ok(msg) = rx.recv().await {
            if let Err(e) = send_ws_message(&mut socket, &msg).await {
                warn!("Failed to send message, closing connection: {}", e);
                break;
            }
        }
    });
}