use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};
use tracing::{debug, error, info, warn};

use crate::{http::AppState, wol::send_magic_packet};
use shuthost_common::{create_hmac_message, sign_hmac, verify_hmac, is_timestamp_in_valid_range};

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(list_nodes))
        .route("/wake/{hostname}", post(wake_host))
        .route("/shutdown/{hostname}", post(shutdown_host))
        .route("/status/{hostname}", get(status_host))
        .route("/m2m/lease/{hostname}/{action}", post(handle_lease))
}

async fn list_nodes(State(AppState { config_rx, .. }): State<AppState>) -> impl IntoResponse {
    let config = config_rx.borrow();
    let hosts: Vec<_> = config
        .nodes
        .iter()
        .map(|(name, node)| {
            json!({
                "name": name,
                "ip": node.ip,
                "mac": node.mac,
                "port": node.port,
            })
        })
        .collect();

    Json(hosts)
}

async fn status_host(
    Path(hostname): Path<String>,
    State(AppState { is_on_rx, .. }): State<AppState>,
) -> impl IntoResponse {
    let is_on_rx = is_on_rx.borrow();
    match is_on_rx.get(&hostname) {
        Some(status) => {
            debug!("Status check for '{}': {}", hostname, status);
            match *status {
                true => "online",
                false => "offline",
            }
            .into_response()
        }
        None => {
            warn!("Status check for unknown host '{}'", hostname);
            (StatusCode::NOT_FOUND, "Unknown host").into_response()
        }
    }
}

async fn wake_host(
    Path(hostname): Path<String>,
    State(AppState { config_rx, .. }): State<AppState>,
) -> impl IntoResponse {
    let host = {
        let config = config_rx.borrow();
        let Some(host) = config.nodes.get(&hostname) else {
            warn!("Wake request for unknown host '{}'", hostname);
            return (StatusCode::NOT_FOUND, "Unknown host").into_response();
        };
        host.clone()
    };
    // let magic_packet_relay = &host.ip;
    let magic_packet_relay = "255.255.255.255";
    match send_magic_packet(&host.mac, magic_packet_relay) {
        Ok(_) => {
            let info = format!(
                "Magic packet sent to {} via {}",
                &host.mac, magic_packet_relay
            );
            info!(info);
            info.into_response()
        }
        Err(e) => {
            error!("Failed to send magic packet to '{}': {}", hostname, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response()
        }
    }
}

async fn shutdown_host(
    Path(hostname): Path<String>,
    State(AppState { config_rx, .. }): State<AppState>,
) -> impl IntoResponse {
    let node = {
        let config = config_rx.borrow();
        let Some(node) = config.nodes.get(&hostname) else {
            warn!("Shutdown request for unknown host '{}'", hostname);
            return (StatusCode::NOT_FOUND, "Unknown host").into_response();
        };
        node.clone()
    };

    let message = create_hmac_message("shutdown");
    let signature = sign_hmac(&message, &node.shared_secret);
    let full_message = format!("{}|{}", message, signature);

    info!("Sending shutdown command to '{}'", hostname);
    match send_shutdown(&node.ip, node.port, &full_message).await {
        Ok(resp) => {
            info!("Shutdown response from '{}': {}", hostname, resp);
            resp.into_response()
        }
        Err(e) => {
            error!("Failed to shutdown '{}': {}", hostname, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
        }
    }
}

async fn send_shutdown(ip: &str, port: u16, message: &str) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    debug!("Connecting to {}", addr);
    let mut stream = TcpStream::connect(addr).await.map_err(|e| {
        error!("TCP connect error: {}", e);
        e.to_string()
    })?;
    stream.writable().await.map_err(|e| {
        error!("Stream not writable: {}", e);
        e.to_string()
    })?;
    debug!("Sending shutdown message...");
    stream.write_all(message.as_bytes()).await.map_err(|e| {
        error!("Write failed: {}", e);
        e.to_string()
    })?;

    let mut buf = vec![0; 1024];
    let n = stream.read(&mut buf).await.map_err(|e| {
        error!("Read failed: {}", e);
        e.to_string()
    })?;

    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

/// node_name => set of client_ids holding lease
pub type LeaseMap = Arc<Mutex<HashMap<String, HashSet<String>>>>;

#[axum::debug_handler]
async fn handle_lease(
    Path((node, action)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
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

    if is_timestamp_in_valid_range(timestamp) {
        return Err((StatusCode::UNAUTHORIZED, "Timestamp out of range"));
    }

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

    let message = format!("{}|{}", timestamp_str, command);
    if !verify_hmac(&message, signature, &shared_secret) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid HMAC signature"));
    }

    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(node.clone()).or_default();

    // TODO: Implement taking actual action based on leases (shutdown etc)

    match action.as_str() {
        "take" => {
            lease_set.insert(client_id.to_string());
            info!("Client '{}' took lease on '{}'", client_id, node);
            Ok("Lease taken".into_response())
        }
        "release" => {
            lease_set.remove(client_id);
            info!("Client '{}' released lease on '{}'", client_id, node);
            Ok("Lease released".into_response())
        }
        _ => Err((StatusCode::BAD_REQUEST, "Invalid action")),
    }
}
