use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tracing::{debug, error, info, warn};

use crate::{
    http::AppState,
    routes::m2m::{broadcast_lease_update, handle_node_state},
};

use super::m2m::m2m_routes;

pub use super::m2m::{LeaseMap, LeaseSource};

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(list_nodes))
        .nest("/m2m", m2m_routes())
        .route("/lease/{hostname}/take", post(take_lease))
        .route("/lease/{hostname}/release", post(release_lease))
        .route("/status/{hostname}", get(status_host))
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
    State(AppState { hoststatus_rx, .. }): State<AppState>,
) -> impl IntoResponse {
    let is_on_rx = hoststatus_rx.borrow();
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

async fn take_lease(
    Path(hostname): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(hostname.clone()).or_default();
    lease_set.insert(LeaseSource::WebInterface);

    info!("Web interface took lease on '{}'", hostname);

    // Broadcast lease update to WebSocket clients
    broadcast_lease_update(&hostname, lease_set, &state.ws_tx).await;

    // Handle node state after lease change
    if let Err((status, msg)) = handle_node_state(&hostname, &lease_set, &state).await {
        return (status, msg).into_response();
    }

    "Lease taken".into_response()
}

async fn release_lease(
    Path(hostname): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(hostname.clone()).or_default();
    lease_set.remove(&LeaseSource::WebInterface);

    info!("Web interface released lease on '{}'", hostname);

    // Broadcast lease update to WebSocket clients
    broadcast_lease_update(&hostname, lease_set, &state.ws_tx).await;

    // Handle node state after lease change
    if let Err((status, msg)) = handle_node_state(&hostname, &lease_set, &state).await {
        return (status, msg).into_response();
    }

    "Lease released".into_response()
}

pub async fn send_shutdown(ip: &str, port: u16, message: &str) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    debug!("Connecting to {}", addr);

    const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
    let mut stream = match timeout(REQUEST_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            error!("TCP connect error: {}", e);
            return Err(e.to_string());
        }
        Err(e) => {
            error!("Connection timed out: {}", e);
            return Err("Connection timed out".to_string());
        }
    };

    if let Err(e) = timeout(REQUEST_TIMEOUT, stream.writable()).await {
        error!("Stream not writable: {}", e);
        return Err("Stream not writable".to_string());
    }

    debug!("Sending shutdown message...");
    if let Err(e) = timeout(REQUEST_TIMEOUT, stream.write_all(message.as_bytes())).await {
        error!("Write failed: {}", e);
        return Err("Write failed".to_string());
    }

    let mut buf = vec![0; 1024];
    let n = match timeout(REQUEST_TIMEOUT, stream.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            error!("Read failed: {}", e);
            return Err("Read failed".to_string());
        }
        Err(e) => {
            error!("Read timed out: {}", e);
            return Err("Read timed out".to_string());
        }
    };

    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}
