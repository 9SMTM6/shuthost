use core::error;

use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    db,
    http::m2m::{broadcast_lease_update, spawn_handle_host_state},
    state::AppState,
    websocket::LeaseSources,
};

pub(crate) use super::m2m::LeaseSource;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
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
pub(crate) enum LeaseAction {
    Take,
    Release,
}

/// Updates the lease set for a host, persists to database if available, and broadcasts the update.
/// Returns the updated lease set.
pub(crate) async fn update_lease_and_broadcast(
    hostname: &str,
    lease_source: LeaseSource,
    action: LeaseAction,
    state: &AppState,
) -> Result<LeaseSources, Box<dyn error::Error + Send + Sync>> {
    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(hostname.to_string()).or_default();

    use LeaseAction as LA;

    match action {
        LA::Take => {
            lease_set.insert(lease_source.clone());
            info!("{} took lease on '{}'", lease_source, hostname);
            if let Some(ref pool) = state.db_pool {
                db::add_lease(pool, hostname, &lease_source).await?;
            }
        }
        LA::Release => {
            lease_set.remove(&lease_source);
            info!("{} released lease on '{}'", lease_source, hostname);
            if let Some(ref pool) = state.db_pool {
                db::remove_lease(pool, hostname, &lease_source).await?;
            }
        }
    }

    let lease_set = lease_set.clone();
    broadcast_lease_update(hostname, &lease_set, &state.ws_tx).await;

    Ok(lease_set)
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
    let lease_source = LeaseSource::WebInterface;
    let lease_set = match update_lease_and_broadcast(&hostname, lease_source, action, &state).await
    {
        Ok(set) => set,
        Err(e) => {
            error!("Failed to update lease: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Handle host state after lease change
    spawn_handle_host_state(&hostname, &lease_set, &state);

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

    // Remove all leases associated with the client from memory
    for lease_set in leases.values_mut() {
        lease_set.retain(|lease| !matches!(lease, LeaseSource::Client(id) if id == &client_id));
    }

    // Remove all leases associated with the client from database when enabled
    if let Some(ref pool) = state.db_pool
        && let Err(e) = db::remove_client_leases(pool, &client_id).await
    {
        tracing::error!("Failed to remove client leases from database: {}", e);
    }

    // Broadcast updated lease information to WebSocket clients
    for (host, lease_set) in leases.iter() {
        broadcast_lease_update(host, lease_set, &state.ws_tx).await;
    }

    // Handle host state after lease changes
    for (host, lease_set) in leases.iter() {
        spawn_handle_host_state(host, lease_set, &state);
    }

    format!("All leases for client '{client_id}' have been reset.").into_response()
}

/// Returns the online status of all hosts as a JSON object.
#[axum::debug_handler]
async fn get_hosts_status(State(state): State<AppState>) -> impl IntoResponse {
    let hoststatus = state.hoststatus_rx.borrow().clone();
    axum::Json((*hoststatus).clone())
}
