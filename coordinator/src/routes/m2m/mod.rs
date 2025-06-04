use std::time::Duration;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Display},
    sync::Arc,
};
use tokio::time::sleep;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Deserialize;
use serde_json::json;
use shuthost_common::{create_hmac_message, is_timestamp_in_valid_range, sign_hmac, verify_hmac};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::{http::AppState, wol::send_magic_packet};

use super::api::send_shutdown;

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
        .route("/lease/{hostname}/{action}", post(handle_lease))
        .route("/test_wol", post(test_wol))
}

#[derive(Deserialize)]
pub struct WolTestQuery {
    port: u16,
}

async fn test_wol(
    headers: axum::http::HeaderMap,
    Query(params): Query<WolTestQuery>,
) -> impl IntoResponse {
    let remote_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()))
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "No client IP found").into_response())?;

    match crate::wol::test_wol_reachability(remote_ip, params.port) {
        Ok((direct, broadcast)) => Ok(Json(json!({
            "direct": direct,
            "broadcast": broadcast
        }))
        .into_response()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e).into_response()),
    }
}

/// node_name => set of lease sources holding lease
pub type LeaseMap = Arc<Mutex<HashMap<String, HashSet<LeaseSource>>>>;

#[derive(Deserialize)]
pub struct LeaseActionQuery {
    #[serde(default)]
    r#async: Option<bool>,
}

// Update the handler to accept the query parameter for async operation
#[axum::debug_handler]
async fn handle_lease(
    Path((node, action)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<LeaseActionQuery>,
) -> impl IntoResponse {
    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing X-Client-ID"))?;

    let data_str = headers
        .get("X-Request")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing X-Request"))?;

    let parts: Vec<&str> = data_str.split('|').collect();
    if parts.len() != 3 {
        return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
    }

    let (timestamp_str, command, signature) = (parts[0], parts[1], parts[2]);
    if command != action {
        return Err((StatusCode::BAD_REQUEST, "Action mismatch"));
    }

    let timestamp: u64 = timestamp_str
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid timestamp"))?;

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

    if !is_timestamp_in_valid_range(timestamp) {
        info!("Timestamp out of range for client '{}'", client_id);
        return Err((StatusCode::UNAUTHORIZED, "Timestamp out of range"));
    }

    let message = format!("{}|{}", timestamp_str, command);
    if !verify_hmac(&message, signature, &shared_secret) {
        info!("Invalid HMAC signature for client '{}'", client_id);
        return Err((StatusCode::UNAUTHORIZED, "Invalid HMAC signature"));
    }

    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(node.clone()).or_default();
    let lease_source = LeaseSource::Client(client_id.to_string());

    let is_async = q.r#async.unwrap_or(false);

    match action.as_str() {
        "take" => {
            lease_set.insert(lease_source.clone());
            info!("Client '{}' took lease on '{}'", client_id, node);

            if is_async {
                let node = node.clone();
                let lease_set = lease_set.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    let _ = handle_node_state(&node, &lease_set, &state).await;
                });
                Ok("Lease taken (async)".into_response())
            } else {
                handle_node_state(&node, &lease_set, &state).await?;
                Ok("Lease taken, node is online".into_response())
            }
        }
        "release" => {
            lease_set.remove(&lease_source);
            info!("Client '{}' released lease on '{}'", client_id, node);

            if is_async {
                let node = node.clone();
                let lease_set = lease_set.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    let _ = handle_node_state(&node, &lease_set, &state).await;
                });
                Ok("Lease released (async)".into_response())
            } else {
                handle_node_state(&node, &lease_set, &state).await?;
                Ok("Lease released, node is offline".into_response())
            }
        }
        _ => Err((StatusCode::BAD_REQUEST, "Invalid action")),
    }
}

pub async fn handle_node_state(
    node: &str,
    lease_set: &HashSet<LeaseSource>,
    state: &AppState,
) -> Result<(), (StatusCode, &'static str)> {
    // If there are any leases, the node should be running
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Checking state for node '{}': should_be_running={}, active_leases={:?}",
        node, should_be_running, lease_set
    );

    let mut is_on = {
        let is_on_rx = state.is_on_rx.borrow();
        is_on_rx.get(node).copied().unwrap_or(false)
    };

    debug!("Current state for node '{}': is_on={}", node, is_on);

    if should_be_running && !is_on {
        info!(
            "Node '{}' needs to wake up - has {} active lease(s): {:?}",
            node,
            lease_set.len(),
            lease_set
        );
        wake_node(node, state)?;

        // Wait until node is reported as online, with timeout
        let mut waited = 0;
        let max_wait = 60; // seconds
        let poll_interval = 1; // second
        loop {
            is_on = {
                let is_on_rx = state.is_on_rx.borrow();
                is_on_rx.get(node).copied().unwrap_or(false)
            };
            if is_on {
                info!("Node '{}' is now online", node);
                break;
            }
            if waited >= max_wait {
                warn!("Timeout waiting for node '{}' to become online", node);
                return Err((
                    StatusCode::GATEWAY_TIMEOUT,
                    "Timeout waiting for node to become online",
                ));
            }
            sleep(Duration::from_secs(poll_interval)).await;
            waited += poll_interval;
        }
    } else if !should_be_running && is_on {
        info!("Node '{}' should shut down - no active leases", node);
        shutdown_node(node, state).await?;

        // Wait until node is reported as offline, with timeout
        let mut waited = 0;
        let max_wait = 60; // seconds
        let poll_interval = 1; // second
        loop {
            is_on = {
                let is_on_rx = state.is_on_rx.borrow();
                is_on_rx.get(node).copied().unwrap_or(false)
            };
            if !is_on {
                info!("Node '{}' is now offline", node);
                break;
            }
            if waited >= max_wait {
                warn!("Timeout waiting for node '{}' to become offline", node);
                return Err((
                    StatusCode::GATEWAY_TIMEOUT,
                    "Timeout waiting for node to become offline",
                ));
            }
            sleep(Duration::from_secs(poll_interval)).await;
            waited += poll_interval;
        }
    } else {
        debug!(
            "No action needed for node '{}' (is_on={}, should_be_running={})",
            node, is_on, should_be_running
        );
    }

    Ok(())
}

fn wake_node(node: &str, state: &AppState) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to wake node '{}'", node);

    let host = {
        let config = state.config_rx.borrow();
        match config.nodes.get(node) {
            Some(host) => {
                debug!(
                    "Found configuration for node '{}': ip={}, mac={}",
                    node, host.ip, host.mac
                );
                host.clone()
            }
            None => {
                error!("No configuration found for node '{}'", node);
                return Err((StatusCode::NOT_FOUND, "Unknown host"));
            }
        }
    };

    info!("Sending WoL packet to '{}' (MAC: {})", node, host.mac);
    send_magic_packet(&host.mac, "255.255.255.255").map_err(|e| {
        error!("Failed to send WoL packet to '{}': {}", node, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send wake packet",
        )
    })?;

    info!("Successfully sent WoL packet to '{}'", node);
    Ok(())
}

async fn shutdown_node(node: &str, state: &AppState) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to shutdown node '{}'", node);

    let node_config = {
        let config = state.config_rx.borrow();
        match config.nodes.get(node) {
            Some(config) => {
                debug!(
                    "Found configuration for node '{}': ip={}, port={}",
                    node, config.ip, config.port
                );
                config.clone()
            }
            None => {
                error!("No configuration found for node '{}'", node);
                return Err((StatusCode::NOT_FOUND, "Unknown host"));
            }
        }
    };

    let message = create_hmac_message("shutdown");
    let signature = sign_hmac(&message, &node_config.shared_secret);
    let full_message = format!("{}|{}", message, signature);

    info!(
        "Sending shutdown command to '{}' ({}:{})",
        node, node_config.ip, node_config.port
    );
    send_shutdown(&node_config.ip, node_config.port, &full_message)
        .await
        .map_err(|e| {
            error!("Failed to send shutdown command to '{}': {}", node, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send shutdown command",
            )
        })?;

    info!("Successfully sent shutdown command to '{}'", node);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LeaseSource {
    WebInterface,
    Client(String),
}

impl Display for LeaseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaseSource::WebInterface => write!(f, "web-interface"),
            LeaseSource::Client(id) => write!(f, "client-{}", id),
        }
    }
}
