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
};
use tracing::{debug, error, info, warn};

use crate::{http::AppState, wol::send_magic_packet};
use shuthost_common::{create_hmac_message, sign_hmac};

use super::m2m::m2m_routes;

pub use super::m2m::LeaseMap;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(list_nodes))
        .nest("/m2m", m2m_routes())
        .route("/wake/{hostname}", post(wake_host))
        .route("/shutdown/{hostname}", post(shutdown_host))
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
