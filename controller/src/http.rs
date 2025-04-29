use std::{net::SocketAddr, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use axum::{
    routing::{get, post},
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::json;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use tokio::time::timeout;
use std::time::Duration;

use crate::config::{load_controller_config, ControllerConfig};
use crate::wol::send_magic_packet;

type AppState = Arc<ControllerConfig>;

pub async fn start_http_server(config_path: &std::path::Path) {
    let config = Arc::new(load_controller_config(config_path).expect("Failed to load config"));

    let app = Router::new()
        .route("/api/hosts", get(list_hosts))
        .route("/api/wake/{hostname}", post(wake_host))
        .route("/api/shutdown/{hostname}", post(shutdown_host))
        .route("/api/status/{hostname}", get(status_host))
        .route("/download_agent/macos", get(download_agent_macos))
        .route("/download_agent/linux", get(download_agent_linux))
        .route("/", get(serve_ui))
        .with_state(config);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8081));
    println!("Listening on http://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}

async fn list_hosts(State(config): State<AppState>) -> impl IntoResponse {
    let hosts: Vec<_> = config.hosts.iter().map(|(name, host)| {
        json!({
            "name": name,
            "ip": host.ip,
            "mac": host.mac,
            "port": host.port,
        })
    }).collect();

    Json(hosts)
}

async fn status_host(Path(hostname): Path<String>, State(config): State<AppState>) -> impl IntoResponse {
    match config.hosts.get(&hostname) {
        Some(host) => {
            let addr = format!("{}:{}", host.ip, host.port);
            match timeout(Duration::from_millis(200), TcpStream::connect(&addr)).await {
                Ok(Ok(_)) => "online".into_response(),
                _ => "offline".into_response(),
            }
        }
        None => (StatusCode::NOT_FOUND, "Unknown host").into_response(),
    }
}

async fn wake_host(Path(hostname): Path<String>, State(config): State<AppState>) -> impl IntoResponse {
    match config.hosts.get(&hostname) {
        Some(host) => {
            match send_magic_packet(&host.mac, "255.255.255.255") {
                Ok(_) => format!("Magic packet sent to {}", hostname).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response(),
            }
        }
        None => (StatusCode::NOT_FOUND, "Unknown host").into_response(),
    }
}

async fn shutdown_host(Path(hostname): Path<String>, State(config): State<AppState>) -> impl IntoResponse {
    match config.hosts.get(&hostname) {
        Some(host) => {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let message = format!("{}|shutdown", timestamp);
            let signature = sign_hmac(&message, &host.shared_secret);
            let full_message = format!("{}|{}", message, signature);

            match send_shutdown(&host.ip, host.port, &full_message).await {
                Ok(resp) => resp.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            }
        }
        None => (StatusCode::NOT_FOUND, "Unknown host").into_response(),
    }
}

fn sign_hmac(message: &str, secret: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("Invalid key");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn send_shutdown(ip: &str, port: u16, message: &str) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    let mut stream = TcpStream::connect(addr).await.map_err(|e| e.to_string())?;
    stream.writable().await.map_err(|e| e.to_string())?;
    stream.write_all(message.as_bytes()).await.map_err(|e| e.to_string())?;

    let mut buf = vec![0; 1024];

    let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;

    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

async fn serve_ui() -> impl IntoResponse {
    let html = include_str!("../index.html");
    Response::builder()
        .header("Content-Type", "text/html")
        .body(html.into_response())
        .unwrap()
}

async fn download_agent_macos() -> impl IntoResponse {
    download_agent(include_bytes!("../../target/aarch64-apple-darwin/release/shuthost_agent")).await
}

async fn download_agent_linux() -> impl IntoResponse {
    download_agent(include_bytes!("../../target/x86_64-unknown-linux-gnu/release/shuthost_agent")).await
}

async fn download_agent(agent_binary: &'static [u8]) -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", agent_binary.len().to_string())
        .status(StatusCode::OK)
        .body(agent_binary.into_response())
        .unwrap()
}
