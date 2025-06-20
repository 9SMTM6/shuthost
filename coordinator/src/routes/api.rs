use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
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
        .route(
            "/reset_leases/{client_id}",
            post(handle_reset_client_leases),
        )
        .route("/hosts_status", get(get_hosts_status))
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

// TODO: this aint pretty. Maybe invert client/host relationship in LeaseMap.
// TODO: also clean up when a client gets removed from config
// TODO: This fix-all-state approach leads to an eventual sync of host state to leases. Consider making this regular behavior.
/// This function is used by the web UI to reset all leases associated with a client.
/// It does not require any client authentication or HMAC signature.
#[axum::debug_handler]
async fn handle_reset_client_leases(
    Path(client_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut leases = state.leases.lock().await;

    // Remove all leases associated with the client
    for lease_set in leases.values_mut() {
        lease_set.retain(|lease| match lease {
            &LeaseSource::Client(ref id) => id != &client_id,
            _ => true,
        });
    }

    // Broadcast updated lease information to WebSocket clients
    for (host, lease_set) in leases.iter() {
        broadcast_lease_update(host, lease_set, &state.ws_tx).await;
    }

    // Handle host state after lease changes
    for (host, lease_set) in leases.iter() {
        let host = host.clone();
        let lease_set = lease_set.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let _ = handle_host_state(&host, &lease_set, &state).await;
        });
    }

    format!("All leases for client '{}' have been reset.", client_id).into_response()
}

/// Returns the online status of all hosts as a JSON object.
async fn get_hosts_status(State(state): State<AppState>) -> impl IntoResponse {
    let hoststatus = state.hoststatus_rx.borrow().clone();
    axum::Json((*hoststatus).clone())
}
