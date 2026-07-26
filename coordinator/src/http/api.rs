//! Frontend API endpoints (`/api/*`).
//!
//! These endpoints are the backend for the browser SPA.  They are also
//! exposed to M2M clients via HMAC authentication as a **fallback** with
//! **no stability guarantees**.  Production M2M automation should use the
//! stable `/api/m2m/*` endpoints instead.
//!
//! When `AuthInfo::M2MClient` is extracted, the lease source is attributed
//! to the specific client (`LeaseSource::Client(client_id)`) instead of
//! the generic `LeaseSource::WebInterface`.

use core::{
    convert::Infallible,
    fmt::{self, Display},
};

use axum::{
    Extension, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::{TypedHeader, headers::ContentType};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::{
    app::{AppState, LeaseSource, db, lookup_host},
    http::auth::AuthInfo,
    include_utf8_asset,
};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/lease/{hostname}/{action}", post(handle_web_lease_action))
        .route(
            "/reset_leases/{client_id}",
            post(handle_reset_client_leases),
        )
        .route("/hosts_status", get(get_hosts_status))
        .route("/dependency-data.json", get(serve_dependency_data))
        .route("/update", get(get_latest_release))
}

/// Returns the latest GitHub release if a newer version than the running one is available,
/// or `null` if the running version is already up to date (or the check has not completed yet).
#[axum::debug_handler]
async fn get_latest_release(
    State(state): State<AppState>,
    _auth: Option<Extension<AuthInfo>>,
) -> impl IntoResponse {
    axum::Json(state.latest_release.read().await.clone())
}

#[axum::debug_handler]
async fn serve_dependency_data(
    _auth: Option<Extension<AuthInfo>>,
) -> impl IntoResponse {
    (
        TypedHeader(ContentType::json()),
        include_utf8_asset!("generated/about-data.json"),
    )
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum UpdateLeaseError {
    #[error("Host not found: {hostname}")]
    HostNotFound { hostname: String },
    #[error(transparent)]
    DatabaseError(#[from] sqlx::Error),
}

/// Updates the lease set for a host and persists to database if available.
#[tracing::instrument(skip(state))]
pub(crate) async fn update_lease(
    hostname: &str,
    lease_source: LeaseSource,
    action: LeaseAction,
    state: &AppState,
) -> Result<bool, UpdateLeaseError> {
    // Ensure that the host exists, to avoid creating lease entries for non-existent hosts.
    lookup_host(state, hostname).ok_or_else(|| UpdateLeaseError::HostNotFound {
        hostname: hostname.to_string(),
    })?;
    state
        .leases
        .update({
            let hostname = hostname.to_string();
            let lease_source = lease_source.clone();
            let db_pool = state.db_pool.clone();
            async move |map| {
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
                Ok(lease_set.is_empty())
            }
        })
        .await
}

/// Handles taking or releasing a lease on a host, attributed to the authenticated identity.
///
/// The lease source is determined from the [`AuthInfo`] extension inserted by the auth middleware:
/// - `WebSession` → attributed to `WebInterface` (browser user).
/// - `M2MClient { client_id }` → attributed to `Client(client_id)` (m2m client).
/// - No `AuthInfo` (Disabled/External mode) → default to `WebInterface`.
#[axum::debug_handler]
#[tracing::instrument(skip(state))]
async fn handle_web_lease_action(
    Path((hostname, action)): Path<(String, LeaseAction)>,
    State(state): State<AppState>,
    auth: Option<Extension<AuthInfo>>,
) -> impl IntoResponse {
    let lease_source = match auth {
        Some(Extension(AuthInfo::WebSession)) | None => LeaseSource::WebInterface,
        Some(Extension(AuthInfo::M2MClient { client_id })) => LeaseSource::Client(client_id),
    };
    match update_lease(&hostname, lease_source, action, &state).await {
        Ok(_) => {
            // Reconciler task handles the host control action.
            match action {
                LeaseAction::Take => "Lease taken (async)".into_response(),
                LeaseAction::Release => "Lease released (async)".into_response(),
            }
        }
        Err(UpdateLeaseError::HostNotFound { .. }) => {
            warn!("Attempted to {action:?} lease for unknown host: {hostname}",);
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(e) => {
            error!("Failed to update lease: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
}

/// Resets all leases associated with a client. Accepts both web sessions and m2m clients.
#[axum::debug_handler]
#[tracing::instrument(skip(state))]
async fn handle_reset_client_leases(
    Path(client_id): Path<String>,
    State(state): State<AppState>,
    _auth: Option<Extension<AuthInfo>>,
) -> impl IntoResponse {
    state
        .leases
        .update({
            let client_id = client_id.clone();
            let db_pool = state.db_pool.clone();
            async move |map| {
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
            }
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
async fn get_hosts_status(
    State(state): State<AppState>,
    _auth: Option<Extension<AuthInfo>>,
) -> impl IntoResponse {
    let hoststatus = state.host_actor.borrow().clone();
    axum::Json((*hoststatus).clone())
}
