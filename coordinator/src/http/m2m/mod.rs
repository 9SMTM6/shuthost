//! Machine-to-machine API endpoints for lease management and host control.
#![cfg_attr(
    coverage,
    expect(unused_imports, reason = "For some reason clippy sets coverage cfg?"),
    expect(dead_code, reason = "For some reason clippy sets coverage cfg?")
)]

mod validation;

use core::{iter, time::Duration};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use chrono::Utc;
use serde_json::json;
use tokio::time::Instant;
use tracing::{debug, error};

use crate::{
    app::{
        AppState, HostControlError, HostState, LeaseSource, db, lookup_host_with_overrides,
        poll_and_wait,
    },
    http::api::{LeaseAction, UpdateLeaseError, update_lease},
    websocket::WsMessage,
    wol,
};

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
    match wol::test_wol_reachability(params.port) {
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
#[tracing::instrument(skip(headers, state, query))]
async fn handle_m2m_lease_action(
    Path((host, action)): Path<(String, LeaseAction)>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<LeaseActionQuery>,
) -> impl IntoResponse {
    let client_id = match validation::validate_m2m_request(&headers, &state, action) {
        Ok(res) => res,
        Err((sc, err)) => return Err((sc, err.to_owned())),
    };

    tracing::info!(%client_id, "Accepted m2m request");

    // Update client's last used timestamp
    if let Some(ref pool) = state.db_pool {
        match db::update_client_last_used(pool, &client_id, Utc::now()).await {
            Ok(()) => {
                let info = db::get_client_stats(pool, &client_id).await;
                match info {
                    Ok(Some(info)) => {
                        if let Err(_err) = state.ws_tx.send(WsMessage::ClientStats(
                            iter::once((client_id.clone(), info)).collect(),
                        )) {
                            debug!("No Websocket Subscribers for client stats");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::error!("Failed to load updated client stats: {}", e),
                }
            }
            Err(e) => tracing::error!("Failed to update client last used: {}", e),
        }
    }

    let lease_source = LeaseSource::Client(client_id);

    let is_async = query.r#async.unwrap_or(false);

    use LeaseAction as LA;
    use StatusCode as SC;
    use UpdateLeaseError as ULE;

    let lease_set_empty = update_lease(&host, lease_source, action, &state)
        .await
        .map_err(|e| match e {
            ULE::HostNotFound { hostname: _ } => (
                SC::NOT_FOUND,
                format!("No configuration found for host {host}"),
            ),
            ULE::DatabaseError(_) => {
                error!("Failed to update lease: {}", e);
                (
                    SC::INTERNAL_SERVER_ERROR,
                    "Failed to update lease".to_string(),
                )
            }
        })?;

    use HostState as HS;
    let ultimately_desired_state = if lease_set_empty {
        HS::Offline
    } else {
        HS::Online
    };

    let current_state = state.hoststatus.get_current_state(&host);
    if current_state == ultimately_desired_state {
        // Short-circuit if the host is already in the desired state.
        return Ok(match (action, ultimately_desired_state) {
            (LA::Take, HS::Online) => "Lease taken, host is already online",
            (LA::Release, HS::Offline) => "Lease released, host is already offline",
            (LA::Release, HS::Online) => "Lease released, but host remains online",
            (LA::Take, HS::Offline) => unreachable!("taking a lease on an offline host should not assure that the lease_set is not empty"),
            (_, _) => unreachable!("we should've handled all possible combinations of (action, ultimately_desired_state) by this point"),
        }
        .into_response());
    }
    if is_async {
        Ok(match action {
            LA::Take => "Lease taken (async)",
            LA::Release => "Lease released (async)",
        }
        .into_response())
    } else {
        // Lookup host config for per-host timeout values.
        let Some(host_with_name) = lookup_host_with_overrides(&state, &host).await else {
            return Err((
                SC::NOT_FOUND,
                format!("No configuration found for host {host}"),
            ));
        };

        let timeout_secs = if ultimately_desired_state == HS::Online {
            host_with_name
                .host
                .wake_timeout_secs
                .unwrap_or(state.runtime.default_wake_timeout_secs)
        } else {
            host_with_name
                .host
                .shutdown_timeout_secs
                .unwrap_or(state.runtime.default_shutdown_timeout_secs)
        };
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        match poll_and_wait(
            &host_with_name,
            &state.hoststatus,
            ultimately_desired_state,
            deadline,
            &state.runtime,
        )
        .await
        {
            Ok(()) => {
                // Host reached the desired state within the timeout.
                Ok(match (action, ultimately_desired_state) {
                    (LA::Take, HS::Online) => "Lease taken, host is now online",
                    (LA::Release, HS::Offline) => "Lease released, host is now offline",
                    _ => unreachable!("unexpected (action, ultimately_desired_state) combination"),
                }
                .into_response())
            }
            Err(err) => {
                use HostControlError as HCE;
                Err((
                    match err {
                        HCE::NotFound(_) => SC::NOT_FOUND,
                        HCE::Timeout(_) => SC::GATEWAY_TIMEOUT,
                        HCE::OperationFailed { .. } => SC::INTERNAL_SERVER_ERROR,
                    },
                    err.to_string(),
                ))
            }
        }
    }
    // In async mode, the lease map update already published a watch event;
    // the reconciler background task will handle the host control action.
    // Ok(match (action, desired_state) {
    //     (LA::Take, HS::Online) => "Lease taken, host is online",
    //     (LA::Take, HS::Waking) => "Lease taken (async)",
    //     (LA::Release, HS::Online) => "Lease released, host remains online",
    //     (LA::Release, HS::Offline) => "Lease released, host is offline",
    //     (LA::Release, HS::ShuttingDown) => "Lease released (async)",
    //     _ => unreachable!("unexpected (action, desired_state) combination"),
    // })
}
