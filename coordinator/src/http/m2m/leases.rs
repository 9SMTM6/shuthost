//! Lease management types and utilities.

use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast};
use tracing::debug;

use crate::websocket::WsMessage;

/// host_name => set of lease sources holding lease
pub type LeaseMap = Arc<Mutex<HashMap<String, HashSet<LeaseSource>>>>;

/// Represents a source that holds a lease on a host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LeaseSource {
    /// Lease held by the web interface
    WebInterface,
    /// Lease held by a specific client
    Client(String),
}

impl Display for LeaseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            LeaseSource::WebInterface => write!(f, "web-interface"),
            LeaseSource::Client(ref id) => write!(f, "client-{id}"),
        }
    }
}

/// Broadcast a lease update to WebSocket clients.
pub async fn broadcast_lease_update(
    host: &str,
    leases: &HashSet<LeaseSource>,
    ws_tx: &broadcast::Sender<WsMessage>,
) {
    let msg = WsMessage::LeaseUpdate {
        host: host.to_string(),
        leases: leases.clone(),
    };
    if ws_tx.send(msg).is_err() {
        debug!("No Websocket Subscribers");
    }
}
