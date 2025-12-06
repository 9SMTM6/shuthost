//! Machine-to-machine API endpoints for lease management and host control.
#![cfg_attr(
    coverage,
    expect(unused_imports, reason = "For some reason clippy sets coverage cfg?"),
    expect(dead_code, reason = "For some reason clippy sets coverage cfg?")
)]

mod host_control;
mod leases;
mod validation;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use chrono::Utc;
use serde_json::json;
use tracing::info;

use crate::http::AppState;
use crate::http::api::LeaseAction;

// Re-export public API
pub(crate) use host_control::handle_host_state;
pub(crate) use leases::{LeaseMap, LeaseSource, broadcast_lease_update};

pub(crate) fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/lease/{hostname}/{action}", post(handle_m2m_lease_action))
        .route("/test_wol", post(test_wol))
}

#[derive(serde::Deserialize)]
pub(crate) struct WolTestQuery {
    port: u16,
}

#[cfg(not(coverage))]
#[axum::debug_handler]
async fn test_wol(Query(params): Query<WolTestQuery>) -> impl IntoResponse {
    match crate::wol::test_wol_reachability(params.port) {
        Ok(broadcast) => Ok(Json(json!({
            "broadcast": broadcast
        }))
        .into_response()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
    }
}

#[cfg(coverage)]
#[axum::debug_handler]
async fn test_wol() -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unimplemented in coverage",
    )
        .into_response()
}

#[derive(serde::Deserialize)]
pub(crate) struct LeaseActionQuery {
    #[serde(default)]
    r#async: Option<bool>,
}

/// Handles machine-to-machine lease actions (take/release) for a host.
///
/// This endpoint is intended for programmatic (m2m) clients and requires additional
/// authorization via HMAC-signed headers. The client must provide a valid `X-Client-ID`
/// and a signed `X-Request` header containing a timestamp, command, and signature.
///
/// The `action` path parameter must be either `take` or `release` and is mapped to the `LeaseAction` enum.
///
/// The `async` query parameter determines whether the host state change (wake/shutdown) is performed
/// synchronously (the request waits for the host to reach the desired state, up to a timeout) or asynchronously
/// (the request returns immediately after triggering the state change, and the host may still be transitioning).
///
/// - In synchronous mode (default), the request will block until the host is confirmed online (for take) or offline (for release),
///   or until a timeout is reached. This provides strong guarantees to the client about the host's state at the time of response.
/// - In asynchronous mode (`?async=true`), the request returns immediately after triggering the state change, and the host may still
///   be transitioning. This is useful for clients that want a fast response and can poll for state changes separately.
///
/// This is distinct from the web interface lease endpoints, which do not require authentication and are used for
/// user-initiated actions from the web UI. Use this endpoint for secure, automated lease management by trusted clients.
#[axum::debug_handler]
async fn handle_m2m_lease_action(
    Path((host, action)): Path<(String, LeaseAction)>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<LeaseActionQuery>,
) -> impl IntoResponse {
    let client_id = match validation::validate_m2m_request(&headers, &state, &action) {
        Ok(res) => res,
        Err((sc, err)) => return Err((sc, err.to_owned())),
    };

    // Update client's last used timestamp
    if let Some(ref pool) = state.db_pool
        && let Err(e) = crate::db::update_client_last_used(pool, &client_id, Utc::now()).await
    {
        tracing::error!("Failed to update client last used: {}", e);
    }

    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(host.clone()).or_default();
    let lease_source = leases::LeaseSource::Client(client_id);

    let is_async = q.r#async.unwrap_or(false);

    match action {
        LeaseAction::Take => {
            lease_set.insert(lease_source.clone());
            leases::broadcast_lease_update(&host, lease_set, &state.ws_tx).await;
            info!("Client '{}' took lease on '{}'", lease_source, host);
            // Persist to database when enabled
            if let Some(ref pool) = state.db_pool
                && let Err(e) = crate::db::add_lease(pool, &host, &lease_source).await
            {
                tracing::error!("Failed to persist lease change: {}", e);
            }

            if is_async {
                // In async mode, the host state change is triggered in the background and the response returns immediately.
                // The host may still be transitioning to the online state when the client receives the response.
                let host = host.clone();
                let lease_set = lease_set.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    drop(host_control::handle_host_state(&host, &lease_set, &state).await);
                });
                Ok("Lease taken (async)".into_response())
            } else {
                // In sync mode, the request waits for the host to reach the online state (or timeout) before returning.
                host_control::handle_host_state(&host, lease_set, &state).await?;
                Ok("Lease taken, host is online".into_response())
            }
        }
        LeaseAction::Release => {
            lease_set.remove(&lease_source);
            leases::broadcast_lease_update(&host, lease_set, &state.ws_tx).await;
            info!("Client '{}' released lease on '{}'", lease_source, host);
            // Persist to database when enabled
            if let Some(ref pool) = state.db_pool
                && let Err(e) = crate::db::remove_lease(pool, &host, &lease_source).await
            {
                tracing::error!("Failed to persist lease change: {}", e);
            }

            if is_async {
                // In async mode, the host state change is triggered in the background and the response returns immediately.
                // The host may still be transitioning to the offline state when the client receives the response.
                let host = host.clone();
                let lease_set = lease_set.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    drop(host_control::handle_host_state(&host, &lease_set, &state).await);
                });
                Ok("Lease released (async)".into_response())
            } else {
                // In sync mode, the request waits for the host to reach the offline state (or timeout) before returning.
                host_control::handle_host_state(&host, lease_set, &state).await?;
                Ok("Lease released, host is offline".into_response())
            }
        }
    }
}
