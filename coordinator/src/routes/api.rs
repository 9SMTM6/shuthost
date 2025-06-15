use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    http::AppState,
    routes::m2m::{broadcast_lease_update, handle_host_state},
};

use super::m2m::m2m_routes;

pub use super::m2m::{LeaseMap, LeaseSource};

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .nest("/m2m", m2m_routes())
        .route("/lease/{hostname}/{action}", post(handle_web_lease_action))
}

/// Lease action for lease endpoints (shared between web and m2m)
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LeaseAction {
    Take,
    Release,
}

/// Handles taking or releasing a lease on a host via the web interface.
///
/// This function is used by the web UI to take or release a lease on a host. It does not require
/// any client authentication or HMAC signature, unlike the m2m `handle_lease` endpoint.
/// The lease is attributed to the web interface and is visible to all clients.
///
/// Use this for user-initiated actions from the web dashboard. For programmatic or
/// machine-to-machine lease management, use the `/m2m/lease/{hostname}/{action}` endpoint.
#[axum::debug_handler]
async fn handle_web_lease_action(
    Path((hostname, action)): Path<(String, LeaseAction)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(hostname.clone()).or_default();
    match action {
        LeaseAction::Take => {
            lease_set.insert(LeaseSource::WebInterface);
            info!("Web interface took lease on '{}'", hostname);
        }
        LeaseAction::Release => {
            lease_set.remove(&LeaseSource::WebInterface);
            info!("Web interface released lease on '{}'", hostname);
        }
    }

    // Broadcast lease update to WebSocket clients
    broadcast_lease_update(&hostname, lease_set, &state.ws_tx).await;

    let lease_set = lease_set.clone();
    let state = state.clone();

    // Handle host state after lease change
    tokio::spawn(async move {
        let _ = handle_host_state(&hostname, &lease_set, &state).await;
    });

    match action {
        LeaseAction::Take => "Lease taken (async)".into_response(),
        LeaseAction::Release => "Lease released (async)".into_response(),
    }
}
