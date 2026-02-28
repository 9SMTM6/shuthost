//! Machine-to-machine API endpoints for lease management and host control.
#![cfg_attr(
    coverage,
    expect(unused_imports, reason = "For some reason clippy sets coverage cfg?"),
    expect(dead_code, reason = "For some reason clippy sets coverage cfg?")
)]

mod validation;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use chrono::Utc;
use serde_json::json;
use tracing::error;

use crate::{
    app::{
        AppState, HostControlError, LeaseSource, db, handle_host_state, spawn_handle_host_state,
    },
    http::api::{LeaseAction, update_lease_and_broadcast},
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

    tracing::info!(client_id = %client_id, "Accepted m2m request");

    // Update client's last used timestamp
    if let Some(ref pool) = state.db_pool
        && let Err(e) = db::update_client_last_used(pool, &client_id, Utc::now()).await
    {
        tracing::error!("Failed to update client last used: {}", e);
    }

    let lease_source = LeaseSource::Client(client_id);

    let is_async = query.r#async.unwrap_or(false);

    let lease_set = match update_lease_and_broadcast(&host, lease_source, action, &state).await {
        Ok(set) => set,
        Err(e) => {
            error!("Failed to update lease: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update lease".to_string(),
            ));
        }
    };

    use LeaseAction as LA;

    if is_async {
        // In async mode, the host state change is triggered in the background and the response returns immediately.
        // The host may still be transitioning to the desired state when the client receives the response.
        spawn_handle_host_state(&host, &lease_set, &state);
    } else {
        // In sync mode, the request waits for the host to reach the desired state (or timeout) before returning.
        use HostControlError as HCE;
        match handle_host_state(&host, &state, &lease_set).await {
            Ok(()) => {}
            Err(err) => {
                return Err((
                    match err {
                        HCE::NotFound(_) => StatusCode::NOT_FOUND,
                        HCE::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
                        HCE::OperationFailed(_, _) => StatusCode::INTERNAL_SERVER_ERROR,
                    },
                    err.to_string(),
                ));
            }
        }
    }
    Ok(match (action, is_async) {
        (LA::Take, true) => "Lease taken (async)",
        (LA::Take, false) => "Lease taken, host is online",
        (LA::Release, true) => "Lease released (async)",
        (LA::Release, false) => "Lease released, host is offline",
    })
}
