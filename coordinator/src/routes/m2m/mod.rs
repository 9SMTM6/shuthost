use std::time::Duration;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    sync::Arc,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shuthost_common::{create_signed_message, validate_hmac_message};
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, error, info, warn};

use crate::config::Host;
use crate::websocket::WsMessage;
use crate::{http::AppState, wol::send_magic_packet};

use super::api::LeaseAction;

const CLIENT_SCRIPT_TEMPLATE: &str = include_str!("shuthost_client.tmpl.sh");

pub async fn download_client_script() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .body(CLIENT_SCRIPT_TEMPLATE.to_owned())
        .unwrap()
}

pub fn m2m_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/lease/{hostname}/{action}", post(handle_m2m_lease_action))
        .route("/test_wol", post(test_wol))
}

#[derive(Deserialize)]
pub struct WolTestQuery {
    port: u16,
}

async fn test_wol(Query(params): Query<WolTestQuery>) -> impl IntoResponse {
    match crate::wol::test_wol_reachability(params.port) {
        Ok(broadcast) => Ok(Json(json!({
            "broadcast": broadcast
        }))
        .into_response()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e).into_response()),
    }
}

/// host_name => set of lease sources holding lease
pub type LeaseMap = Arc<Mutex<HashMap<String, HashSet<LeaseSource>>>>;

#[derive(Deserialize)]
pub struct LeaseActionQuery {
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
    let (client_id, _command_action) = match validate_m2m_request(&headers, &state, &action) {
        Ok(res) => res,
        Err(e) => return Err(e),
    };

    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(host.clone()).or_default();
    let lease_source = LeaseSource::Client(client_id);

    let is_async = q.r#async.unwrap_or(false);

    match action {
        LeaseAction::Take => {
            lease_set.insert(lease_source.clone());
            broadcast_lease_update(&host, lease_set, &state.ws_tx).await;
            info!("Client '{}' took lease on '{}'", lease_source, host);

            if is_async {
                // In async mode, the host state change is triggered in the background and the response returns immediately.
                // The host may still be transitioning to the online state when the client receives the response.
                let host = host.clone();
                let lease_set = lease_set.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    let _ = handle_host_state(&host, &lease_set, &state).await;
                });
                Ok("Lease taken (async)".into_response())
            } else {
                // In sync mode, the request waits for the host to reach the online state (or timeout) before returning.
                handle_host_state(&host, lease_set, &state).await?;
                Ok("Lease taken, host is online".into_response())
            }
        }
        LeaseAction::Release => {
            lease_set.remove(&lease_source);
            broadcast_lease_update(&host, lease_set, &state.ws_tx).await;
            info!("Client '{}' released lease on '{}'", lease_source, host);

            if is_async {
                // In async mode, the host state change is triggered in the background and the response returns immediately.
                // The host may still be transitioning to the offline state when the client receives the response.
                let host = host.clone();
                let lease_set = lease_set.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    let _ = handle_host_state(&host, &lease_set, &state).await;
                });
                Ok("Lease released (async)".into_response())
            } else {
                // In sync mode, the request waits for the host to reach the offline state (or timeout) before returning.
                handle_host_state(&host, lease_set, &state).await?;
                Ok("Lease released, host is offline".into_response())
            }
        }
    }
}

/// Validates M2M lease action request headers and returns (client_id, LeaseAction)
pub fn validate_m2m_request(
    headers: &axum::http::HeaderMap,
    state: &AppState,
    expected_action: &LeaseAction,
) -> Result<(String, LeaseAction), (StatusCode, &'static str)> {
    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Client-ID"))?;

    let data_str = headers
        .get("X-Request")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Request"))?;

    let parts: Vec<&str> = data_str.split('|').collect();
    if parts.len() != 3 {
        return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
    }

    // potential enumeration issue, if thats something we want to cover.
    let shared_secret = {
        let config = state.config_rx.borrow();
        config
            .clients
            .get(client_id)
            .ok_or_else(|| {
                warn!("Unknown client '{}'", client_id);
                (StatusCode::FORBIDDEN, "Unknown client")
            })?
            .shared_secret
            .clone()
    };

    let command = match validate_hmac_message(data_str, &shared_secret) {
        shuthost_common::HmacValidationResult::Valid(valid_message) => valid_message,
        shuthost_common::HmacValidationResult::InvalidTimestamp => {
            info!("Timestamp out of range for client '{}'", client_id);
            return Err((StatusCode::UNAUTHORIZED, "Timestamp out of range"));
        }
        shuthost_common::HmacValidationResult::InvalidHmac => {
            info!("Invalid HMAC signature for client '{}'", client_id);
            return Err((StatusCode::UNAUTHORIZED, "Invalid HMAC signature"));
        }
        shuthost_common::HmacValidationResult::MalformedMessage => {
            return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
        }
    };

    let command_action: LeaseAction = serde_plain::from_str(&command)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid action in X-Request"))?;

    if &command_action != expected_action {
        return Err((StatusCode::BAD_REQUEST, "Action mismatch"));
    }

    Ok((client_id.to_string(), command_action))
}

pub async fn broadcast_lease_update(
    host: &str,
    lease_set: &HashSet<LeaseSource>,
    ws_tx: &broadcast::Sender<WsMessage>,
) {
    let lease_sources: Vec<_> = lease_set.iter().cloned().collect();
    let msg = WsMessage::LeaseUpdate {
        host: host.to_string(),
        leases: lease_sources,
    };
    if let Err(e) = ws_tx.send(msg) {
        warn!("Failed to broadcast lease update for '{}': {}", host, e);
    }
}

/// Waits for a host to reach the desired state (online/offline) within a timeout.
/// Returns `Ok(())` if the desired state is reached, or an error if the timeout is exceeded.
async fn wait_for_host_state(
    host: &str,
    state: &AppState,
    desired_state: bool,
    timeout_secs: u64,
    poll_interval_secs: u64,
) -> Result<(), (StatusCode, &'static str)> {
    let mut waited = 0;

    loop {
        let current_state = {
            let hoststatus_rx = state.hoststatus_rx.borrow();
            hoststatus_rx.get(host).copied().unwrap_or(false)
        };

        if current_state == desired_state {
            info!(
                "Host '{}' is now {}",
                host,
                if desired_state { "online" } else { "offline" }
            );
            return Ok(());
        }

        if waited >= timeout_secs {
            warn!(
                "Timeout waiting for host '{}' to become {}",
                host,
                if desired_state { "online" } else { "offline" }
            );
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                if desired_state {
                    "Timeout waiting for host to become online"
                } else {
                    "Timeout waiting for host to become offline"
                },
            ));
        }

        sleep(Duration::from_secs(poll_interval_secs)).await;
        waited += poll_interval_secs;
    }
}

// Refactored usage in handle_host_state
pub async fn handle_host_state(
    host: &str,
    lease_set: &HashSet<LeaseSource>,
    state: &AppState,
) -> Result<(), (StatusCode, &'static str)> {
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Checking state for host '{}': should_be_running={}, active_leases={:?}",
        host, should_be_running, lease_set
    );

    let host_is_on = {
        let hoststatus_rx = state.hoststatus_rx.borrow();
        hoststatus_rx.get(host).copied().unwrap_or(false)
    };

    debug!("Current state for host '{}': is_on={}", host, host_is_on);

    if should_be_running && !host_is_on {
        info!(
            "Host '{host}' needs to wake up - has {} active lease(s): {:?}",
            lease_set.len(),
            lease_set
        );
        wake_host(host, state)?;

        wait_for_host_state(host, state, true, 60, 1).await?;
    } else if !should_be_running && host_is_on {
        info!("Host '{host}' should shut down - no active leases");
        shutdown_host(host, state).await?;

        wait_for_host_state(host, state, false, 60, 1).await?;
    } else {
        debug!(
            "No action needed for host '{}' (is_on={}, should_be_running={})",
            host, host_is_on, should_be_running
        );
    }

    Ok(())
}

fn get_host_config(host_name: &str, state: &AppState) -> Result<Host, (StatusCode, &'static str)> {
    let config = state.config_rx.borrow();
    match config.hosts.get(host_name) {
        Some(host) => {
            debug!(
                "Found configuration for host '{}': ip={}, mac={}",
                host_name, host.ip, host.mac
            );
            Ok(host.clone())
        }
        None => {
            error!("No configuration found for host '{}'", host_name);
            Err((StatusCode::NOT_FOUND, "Unknown host"))
        }
    }
}

fn wake_host(host_name: &str, state: &AppState) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to wake host '{}'", host_name);

    let host_config = get_host_config(host_name, state)?;

    info!(
        "Sending WoL packet to '{}' (MAC: {})",
        host_name, host_config.mac
    );
    send_magic_packet(&host_config.mac, "255.255.255.255").map_err(|e| {
        error!("Failed to send WoL packet to '{}': {}", host_name, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send wake packet",
        )
    })?;

    info!("Successfully sent WoL packet to '{}'", host_name);
    Ok(())
}

async fn execute_tcp_shutdown_request(
    stream: &mut TcpStream,
    request: &[u8],
    response_buf: &mut [u8],
) -> Result<usize, String> {
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

    timeout(REQUEST_TIMEOUT, stream.writable())
        .await
        .map_err(|e| format!("Stream not writable (timeout): {e}"))?
        .map_err(|e| format!("Stream not writable: {e}"))?;

    debug!("Sending shutdown message...");
    timeout(REQUEST_TIMEOUT, stream.write_all(request))
        .await
        .map_err(|e| format!("Write failed (timeout): {e}"))?
        .map_err(|e| format!("Write failed: {e}"))?;

    timeout(REQUEST_TIMEOUT, stream.read(response_buf))
        .await
        .map_err(|e| format!("Read timed out: {e}"))?
        .map_err(|e| format!("Read failed: {e}"))
}

pub async fn send_shutdown(ip: &str, port: u16, message: &str) -> Result<String, String> {
    let addr = format!("{ip}:{port}");
    debug!("Connecting to {}", addr);

    const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
    let mut stream = timeout(REQUEST_TIMEOUT, TcpStream::connect(&addr))
        .await
        .map_err(|e| format!("Connection to {addr} timed out: {e}"))?
        .map_err(|e| format!("TCP connect error to {addr}: {e}"))?;

    let mut buf = vec![0; 1024];
    let n = execute_tcp_shutdown_request(&mut stream, message.as_bytes(), &mut buf).await?;

    let Some(data) = buf.get(..n) else {
        unreachable!("Read data size should always be valid, as its >= buffer size");
    };
    Ok(String::from_utf8_lossy(data).to_string())
}

async fn shutdown_host(host: &str, state: &AppState) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to shutdown host '{}'", host);

    let host_config = {
        let config = state.config_rx.borrow();
        match config.hosts.get(host) {
            Some(config) => {
                debug!(
                    "Found configuration for host '{}': ip={}, port={}",
                    host, config.ip, config.port
                );
                config.clone()
            }
            None => {
                error!("No configuration found for host '{}'", host);
                return Err((StatusCode::NOT_FOUND, "Unknown host"));
            }
        }
    };

    info!(
        "Sending shutdown command to '{}' ({}:{})",
        host, host_config.ip, host_config.port
    );
    let signed_message = create_signed_message("shutdown", &host_config.shared_secret);
    send_shutdown(&host_config.ip, host_config.port, &signed_message)
        .await
        .map_err(|e| {
            error!("Failed to send shutdown command to '{}': {}", host, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send shutdown command",
            )
        })?;

    info!("Successfully sent shutdown command to '{}'", host);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LeaseSource {
    WebInterface,
    Client(String),
}

impl Display for LeaseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LeaseSource::WebInterface => write!(f, "web-interface"),
            LeaseSource::Client(ref id) => write!(f, "client-{id}"),
        }
    }
}
