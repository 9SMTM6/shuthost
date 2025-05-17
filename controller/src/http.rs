use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;
use std::{net::IpAddr, time::Duration};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::time::timeout;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{debug, error, info, warn};

use crate::config::{ControllerConfig, load_controller_config};
use crate::wol::send_magic_packet;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use clap::Parser;
use std::collections::HashMap;
use tokio::sync::broadcast;

#[derive(Debug, Parser)]
pub struct ServiceArgs {
    #[arg(
        long = "config",
        env = "SHUTHOST_CONTROLLER_CONFIG_PATH",
        default_value = "shuthost_controller.toml"
    )]
    pub config: String,
}

#[derive(Clone)]
pub struct AppState {
    config_path: std::path::PathBuf,
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    is_on_rx: watch::Receiver<Arc<HashMap<String, bool>>>,
    ws_tx: broadcast::Sender<String>,
}

use tokio::sync::watch;

pub async fn start_http_server(config_path: &std::path::Path) {
    info!("Starting HTTP server...");

    let initial_config = Arc::new(
        load_controller_config(config_path)
            .await
            .expect("Failed to load config"),
    );
    let listen_port = initial_config.server.port;

    let listen_ip: IpAddr = initial_config
        .server
        .bind
        .parse()
        .expect("Invalid bind address");

    let (config_tx, config_rx) = watch::channel(initial_config);

    let initial_status: Arc<HashMap<String, bool>> = Arc::new(HashMap::new());
    let (is_on_tx, is_on_rx) = watch::channel(initial_status);

    let (ws_tx, _) = broadcast::channel(32);

    {
        let config_rx = config_rx.clone();
        let is_on_tx = is_on_tx.clone();
        let ws_tx = ws_tx.clone();
        tokio::spawn(async move {
            poll_host_statuses(config_rx, is_on_tx, ws_tx).await;
        });
    }

    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        let ws_tx = ws_tx.clone();
        tokio::spawn(async move {
            // TODO: warn on changed port
            watch_config_file(path, config_tx, ws_tx).await;
        });
    }

    let app_state = AppState {
        config_rx,
        is_on_rx,
        ws_tx,
        config_path: config_path.to_path_buf(),
    };

    let app = Router::new()
        .route("/api/hosts", get(list_hosts))
        .route("/api/wake/{hostname}", post(wake_host))
        .route("/api/shutdown/{hostname}", post(shutdown_host))
        .route("/api/status/{hostname}", get(status_host))
        .route("/download/installer.sh", get(get_installer))
        .route("/download/agent/macos/aarch64", get(agent_macos_aarch64))
        .route("/download/agent/macos/x86_64", get(agent_macos_x86_64))
        .route("/download/agent/linux/x86_64", get(agent_linux_x86_64))
        .route("/download/agent/linux/aarch64", get(agent_linux_aarch64))
        .route(
            "/download/agent/linux-musl/x86_64",
            get(agent_linux_musl_x86_64),
        )
        .route(
            "/download/agent/linux-musl/aarch64",
            get(agent_linux_musl_aarch64),
        )
        .route("/", get(serve_ui))
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let addr = SocketAddr::from((listen_ip, listen_port));
    info!("Listening on http://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}

async fn watch_config_file(
    path: std::path::PathBuf,
    tx: watch::Sender<Arc<ControllerConfig>>,
    ws_tx: broadcast::Sender<String>,
) {
    use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use tokio::sync::mpsc::unbounded_channel;

    let (raw_tx, mut raw_rx) = unbounded_channel::<Event>();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(event) = res {
                let _ = raw_tx.send(event);
            }
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .expect("Failed to watch config file");

    while let Some(event) = raw_rx.recv().await {
        if matches!(event.kind, EventKind::Modify(_)) {
            info!("Config file modified. Reloading...");
            match load_controller_config(&path).await {
                Ok(new_config) => {
                    let _ = tx.send(Arc::new(new_config));
                    info!("Config reloaded.");
                    let _ = ws_tx.send("config_updated".to_string());
                }
                Err(e) => {
                    error!("Failed to reload config: {}", e);
                }
            }
        }
    }
}

async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    is_on_tx: watch::Sender<Arc<HashMap<String, bool>>>,
    ws_tx: broadcast::Sender<String>,
) {
    loop {
        let config = config_rx.borrow().clone();
        let mut status_map = HashMap::new();

        for (name, host) in &config.hosts {
            let addr = format!("{}:{}", host.ip, host.port);
            let is_online = matches!(
                timeout(Duration::from_millis(200), TcpStream::connect(&addr)).await,
                Ok(Ok(_))
            );
            debug!("Polled {} at {} - online: {}", name, addr, is_online);
            status_map.insert(name.clone(), is_online);
        }

        let _ = ws_tx.send(serde_json::to_string(&status_map).unwrap());
        let _ = is_on_tx.send(Arc::new(status_map));

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn list_hosts(State(AppState { config_rx, .. }): State<AppState>) -> impl IntoResponse {
    let config = config_rx.borrow();
    let hosts: Vec<_> = config
        .hosts
        .iter()
        .map(|(name, host)| {
            json!({
                "name": name,
                "ip": host.ip,
                "mac": host.mac,
                "port": host.port,
            })
        })
        .collect();

    Json(hosts)
}

#[axum::debug_handler]
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

#[axum::debug_handler]
async fn wake_host(
    Path(hostname): Path<String>,
    State(AppState { config_rx, .. }): State<AppState>,
) -> impl IntoResponse {
    let host = {
        let config = config_rx.borrow();
        let Some(host) = config.hosts.get(&hostname) else {
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

#[axum::debug_handler]
async fn shutdown_host(
    Path(hostname): Path<String>,
    State(AppState { config_rx, .. }): State<AppState>,
) -> impl IntoResponse {
    let host = {
        let config = config_rx.borrow();
        let Some(host) = config.hosts.get(&hostname) else {
            warn!("Shutdown request for unknown host '{}'", hostname);
            return (StatusCode::NOT_FOUND, "Unknown host").into_response();
        };
        host.clone()
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let message = format!("{}|shutdown", timestamp);
    let signature = sign_hmac(&message, &host.shared_secret);
    let full_message = format!("{}|{}", message, signature);

    info!("Sending shutdown command to '{}'", hostname);
    match send_shutdown(&host.ip, host.port, &full_message).await {
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

fn sign_hmac(message: &str, secret: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("Invalid key");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
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

async fn serve_ui(State(AppState { config_path, .. }): State<AppState>) -> impl IntoResponse {
    let html = include_str!("../index.html")
        .replace("{controller_config}", &config_path.to_string_lossy());
    Response::builder()
        .header("Content-Type", "text/html")
        .body(html.into_response())
        .unwrap()
}

// Macro to define a handler function from a static binary
macro_rules! agent_handler {
    ($name:ident, $agent_target:expr) => {
        async fn $name() -> impl IntoResponse {
            const AGENT_BINARY: &'static [u8] = include_bytes!(concat!(
                "../../target/",
                $agent_target,
                "/release/shuthost_agent"
            ));
            Response::builder()
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", AGENT_BINARY.len().to_string())
                .status(StatusCode::OK)
                .body(AGENT_BINARY.into_response())
                .unwrap()
        }
    };
}

// Generate all handlers
agent_handler!(agent_macos_aarch64, "aarch64-apple-darwin");
agent_handler!(agent_macos_x86_64, "x86_64-apple-darwin");
agent_handler!(agent_linux_x86_64, "x86_64-unknown-linux-gnu");
agent_handler!(agent_linux_aarch64, "aarch64-unknown-linux-gnu");
agent_handler!(agent_linux_musl_x86_64, "x86_64-unknown-linux-musl");
agent_handler!(agent_linux_musl_aarch64, "aarch64-unknown-linux-musl");

async fn get_installer() -> impl IntoResponse {
    const INSTALLER: &'static [u8] = include_bytes!("./autoinstall.sh");
    Response::builder()
        .header("Content-Type", "text/plain")
        .header("Content-Length", INSTALLER.len().to_string())
        .status(StatusCode::OK)
        .body(INSTALLER.into_response())
        .unwrap()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(AppState { ws_tx, .. }): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, ws_tx.subscribe()))
}

async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if socket.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
}
