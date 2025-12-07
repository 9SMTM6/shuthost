use axum::{
    Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    db,
    http::{
        AppState,
        m2m::{broadcast_lease_update, handle_host_state},
    },
};

pub(crate) use super::m2m::LeaseSource;

/// Lease action for lease endpoints (shared between web and m2m)
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LeaseAction {
    Take,
    Release,
}

/// Response for hosts status endpoint.
#[derive(Serialize)]
pub struct HostsStatusResponse {
    /// Map of host names to their online status.
    pub hosts: std::collections::HashMap<String, bool>,
}

/// Response for VAPID public key endpoint.
#[derive(Serialize)]
pub struct VapidPublicKeyResponse {
    /// The VAPID public key in base64 format.
    pub public_key: String,
}

/// Response for lease action endpoint.
#[derive(Serialize)]
pub struct LeaseActionResponse {
    /// Success message.
    pub message: String,
}

/// Response for reset client leases endpoint.
#[derive(Serialize)]
pub struct ResetClientLeasesResponse {
    /// Success message.
    pub message: String,
}

/// Push subscription request payload.
#[derive(Deserialize)]
pub struct PushSubscriptionRequest {
    /// The push service endpoint URL.
    pub endpoint: String,
    /// The encryption keys.
    pub keys: PushKeys,
}

/// Push subscription keys.
#[derive(Deserialize)]
pub struct PushKeys {
    /// The P-256 DH key for encryption.
    pub p256dh: String,
    /// The auth secret for the subscription.
    pub auth: String,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/lease/{hostname}/{action}", post(handle_web_lease_action))
        .route(
            "/reset_leases/{client_id}",
            post(handle_reset_client_leases),
        )
        .route("/hosts_status", get(get_hosts_status))
        .route("/push/vapid_public_key", get(get_vapid_public_key))
        .route("/push/subscribe", post(subscribe_push))
        .route("/push/unsubscribe", post(unsubscribe_push))
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
    let lease_source = LeaseSource::WebInterface;
    match action {
        LeaseAction::Take => {
            lease_set.insert(lease_source.clone());
            info!("Web interface took lease on '{}'", hostname);
            // Persist to database when enabled
            if let Some(ref pool) = state.db_pool
                && let Err(e) = crate::db::add_lease(pool, &hostname, &lease_source).await
            {
                tracing::error!("Failed to persist lease change: {}", e);
            }
        }
        LeaseAction::Release => {
            lease_set.remove(&lease_source);
            info!("Web interface released lease on '{}'", hostname);
            // Persist to database when enabled
            if let Some(ref pool) = state.db_pool
                && let Err(e) = crate::db::remove_lease(pool, &hostname, &lease_source).await
            {
                tracing::error!("Failed to persist lease change: {}", e);
            }
        }
    }

    // Broadcast lease update to WebSocket clients
    broadcast_lease_update(&hostname, lease_set, &state.ws_tx).await;

    let lease_set = lease_set.clone();
    let state = state.clone();

    // Handle host state after lease change
    tokio::spawn(async move {
        drop(handle_host_state(&hostname, &lease_set, &state).await);
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

    // Remove all leases associated with the client from memory
    for lease_set in leases.values_mut() {
        lease_set.retain(|lease| match lease {
            &LeaseSource::Client(ref id) => id != &client_id,
            _ => true,
        });
    }

    // Remove all leases associated with the client from database when enabled
    if let Some(ref pool) = state.db_pool
        && let Err(e) = crate::db::remove_client_leases(pool, &client_id).await
    {
        tracing::error!("Failed to remove client leases from database: {}", e);
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
            drop(handle_host_state(&host, &lease_set, &state).await);
        });
    }

    format!("All leases for client '{client_id}' have been reset.").into_response()
}

/// Returns the online status of all hosts as a JSON object.
#[axum::debug_handler]
async fn get_hosts_status(State(state): State<AppState>) -> impl IntoResponse {
    let hoststatus = state.hoststatus_rx.borrow().clone();
    axum::Json(HostsStatusResponse {
        hosts: (*hoststatus).clone(),
    })
}

/// Returns the VAPID public key for push notifications.
#[axum::debug_handler]
async fn get_vapid_public_key(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(ref pool) = state.db_pool {
        match db::get_or_generate_vapid_keys(pool).await {
            Ok(vapid_keys) => axum::Json(VapidPublicKeyResponse {
                public_key: vapid_keys.public_key,
            })
            .into_response(),
            Err(e) => {
                tracing::error!("Failed to get VAPID keys: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Database not available").into_response()
    }
}

/// Subscribes to push notifications.
#[axum::debug_handler]
async fn subscribe_push(
    State(state): State<AppState>,
    axum::Json(subscription): axum::Json<PushSubscriptionRequest>,
) -> impl IntoResponse {
    if let Some(ref pool) = state.db_pool {
        let push_subscription = db::PushSubscription {
            endpoint: subscription.endpoint.clone(),
            p256dh: subscription.keys.p256dh,
            auth: subscription.keys.auth,
        };
        match db::store_push_subscription(pool, &push_subscription).await {
            Ok(()) => {
                tracing::info!(
                    "Stored push subscription for endpoint: {}",
                    subscription.endpoint
                );
                StatusCode::OK.into_response()
            }
            Err(e) => {
                tracing::error!("Failed to store push subscription: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Database not available").into_response()
    }
}

/// Unsubscribes from push notifications.
#[axum::debug_handler]
async fn unsubscribe_push(
    State(state): State<AppState>,
    axum::Json(subscription): axum::Json<PushSubscriptionRequest>,
) -> impl IntoResponse {
    if let Some(ref pool) = state.db_pool {
        match db::remove_push_subscription(pool, &subscription.endpoint).await {
            Ok(()) => {
                tracing::info!(
                    "Removed push subscription for endpoint: {}",
                    subscription.endpoint
                );
                StatusCode::OK.into_response()
            }
            Err(e) => {
                tracing::error!("Failed to remove push subscription: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Database not available").into_response()
    }
}
