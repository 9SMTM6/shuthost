//! Lease management types and utilities.

use alloc::sync::Arc;
use core::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::debug;

use crate::{
    state::WsTx,
    websocket::{LeaseMapRaw, LeaseSources, WsMessage},
};

/// See [`LeaseMapRaw`]
pub(crate) type LeaseMap = Arc<Mutex<LeaseMapRaw>>;

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
pub(crate) async fn broadcast_lease_update(host: &str, leases: &LeaseSources, ws_tx: &WsTx) {
    let msg = WsMessage::LeaseUpdate {
        host: host.to_string(),
        leases: leases.clone(),
    };
    if ws_tx.send(msg).is_err() {
        debug!("No Websocket Subscribers");
    }
}
