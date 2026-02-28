use core::convert::Infallible;
use core::{
    error,
    fmt::{self, Display},
};

use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::app::{AppState, LeaseSource, LeaseSources, db};

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
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LeaseAction {
    Take,
    Release,
}

impl Display for LeaseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            LeaseSource::WebInterface => write!(f, "web-interface"),
            LeaseSource::Client(ref id) => write!(f, "client-{id}"),
        }
    }
}

/// Updates the lease set for a host and persists to database if available.
/// Returns the updated lease set.
#[tracing::instrument(skip(state))]
pub(crate) async fn update_lease(
    hostname: &str,
    lease_source: LeaseSource,
    action: LeaseAction,
    state: &AppState,
) -> Result<LeaseSources, Box<dyn error::Error + Send + Sync>> {
    let db_pool = state.db_pool.clone();
    let hostname = hostname.to_string();

    let snapshot = state
        .leases
        .update(|map| {
            let hostname = hostname.clone();
            let lease_source = lease_source.clone();
            let db_pool = db_pool.clone();
            Box::pin(async move {
                let lease_set = map.entry(hostname.clone()).or_default();
                use LeaseAction as LA;
                match action {
                    LA::Take => {
                        lease_set.insert(lease_source.clone());
                        info!(%lease_source, "Lease taken");
                        if let Some(ref pool) = db_pool {
                            db::add_lease(pool, &hostname, &lease_source).await?;
                        }
                    }
                    LA::Release => {
                        lease_set.remove(&lease_source);
                        info!(%lease_source, "Lease released");
                        if let Some(ref pool) = db_pool {
                            db::remove_lease(pool, &hostname, &lease_source).await?;
                        }
                    }
                }
                Ok::<(), Box<dyn error::Error + Send + Sync>>(())
            })
        })
        .await?;

    let lease_set = snapshot.get(&hostname).cloned().unwrap_or_default();

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
#[tracing::instrument(skip(state))]
async fn handle_web_lease_action(
    Path((hostname, action)): Path<(String, LeaseAction)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let lease_source = LeaseSource::WebInterface;
    if let Err(e) = update_lease(&hostname, lease_source, action, &state).await {
        error!("Failed to update lease: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    // Reconciler task handles the host control action.
    match action {
        LeaseAction::Take => "Lease taken (async)".into_response(),
        LeaseAction::Release => "Lease released (async)".into_response(),
    }
}

/// This function is used by the web UI to reset all leases associated with a client.
/// It does not require any client authentication or HMAC signature.
/// The reconciler background task will handle bringing affected hosts to the correct state.
#[axum::debug_handler]
#[tracing::instrument(skip(state))]
async fn handle_reset_client_leases(
    Path(client_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let db_pool = state.db_pool.clone();

    state
        .leases
        .update(|map| {
            let client_id = client_id.clone();
            let db_pool = db_pool.clone();
            Box::pin(async move {
                // Remove all leases associated with the client from memory (atomically)
                for lease_set in map.values_mut() {
                    lease_set.retain(
                        |lease| !matches!(lease, LeaseSource::Client(id) if id == &client_id),
                    );
                }
                // Persist the removal
                if let Some(ref pool) = db_pool
                    && let Err(e) = db::remove_client_leases(pool, &client_id).await
                {
                    tracing::error!("Failed to remove client leases from database: {}", e);
                }
                Ok::<(), Infallible>(())
            })
        })
        .await
        .unwrap_or_else(|e| match e {});

    // Broadcast updated lease information to WebSocket clients
    // (the broadcast_lease_updates background task handles this via the LeaseRx watch channel)

    // Reconciler will handle host control for any newly unleased hosts.

    format!("All leases for client '{client_id}' have been reset.").into_response()
}

/// Returns the online status of all hosts as a JSON object.
#[axum::debug_handler]
async fn get_hosts_status(State(state): State<AppState>) -> impl IntoResponse {
    let hoststatus = state.hoststatus_rx.borrow().clone();
    axum::Json((*hoststatus).clone())
}
