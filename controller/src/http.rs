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
use std::collections::HashMap;

#[derive(Clone)]
pub struct AppState {
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    is_on_rx: watch::Receiver<Arc<HashMap<String, bool>>>,
}

use tokio::sync::watch;

pub async fn start_http_server(config_path: &std::path::Path) {
    let initial_config = Arc::new(
        load_controller_config(config_path).await.expect("Failed to load config"),
    );
    let (config_tx, config_rx) = watch::channel(initial_config);
    
    let initial_status: Arc<HashMap<String, bool>> = Arc::new(HashMap::new());
    let (is_on_tx, is_on_rx) = watch::channel(initial_status);
    
    // Spawn background task
    {
        let config_rx = config_rx.clone();
        let is_on_tx = is_on_tx.clone();
        tokio::spawn(async move {
            poll_host_statuses(config_rx, is_on_tx).await;
        });
    }
    

    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(async move {
            watch_config_file(path, config_tx).await;
        });
    }

    let app_state = AppState { config_rx, is_on_rx };

    let app = Router::new()
        .route("/api/hosts", get(list_hosts))
        .route("/api/wake/{hostname}", post(wake_host))
        .route("/api/shutdown/{hostname}", post(shutdown_host))
        .route("/api/status/{hostname}", get(status_host))
        .route("/download_agent/macos", get(download_agent_macos))
        .route("/download_agent/linux", get(download_agent_linux))
        .route("/", get(serve_ui))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8081));
    println!("Listening on http://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}

async fn watch_config_file(path: std::path::PathBuf, tx: watch::Sender<Arc<ControllerConfig>>) {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
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
            println!("Config file modified. Reloading...");
            match load_controller_config(&path).await {
                Ok(new_config) => {
                    let _ = tx.send(Arc::new(new_config));
                    println!("Config reloaded.");
                }
                Err(e) => {
                    eprintln!("Failed to reload config: {}", e);
                }
            }
        }
    }
}

async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    is_on_tx: watch::Sender<Arc<HashMap<String, bool>>>,
) {
    loop {
        let config = config_rx.borrow().clone();

        let mut status_map = HashMap::new();

        for (name, host) in &config.hosts {
            let addr = format!("{}:{}", host.ip, host.port);
            let status = timeout(Duration::from_millis(200), TcpStream::connect(&addr)).await.is_ok();
            status_map.insert(name.clone(), status);
        }

        let _ = is_on_tx.send(Arc::new(status_map));

        tokio::time::sleep(Duration::from_secs(2)).await; // Adjust interval as needed
    }
}


async fn list_hosts(State(AppState{config_rx, ..}): State<AppState>) -> impl IntoResponse {
    let config = config_rx.borrow();
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

#[axum::debug_handler]
async fn status_host(
    Path(hostname): Path<String>,
    State(AppState { is_on_rx , .. }): State<AppState>,
) -> impl IntoResponse {
    let is_on_rx = is_on_rx.borrow();
    match is_on_rx.get(&hostname) {
        Some(status) => match status.clone() {
            true => "online",
            false => "offline",
        }.into_response(),
        None => (StatusCode::NOT_FOUND, "Unknown host").into_response(),
    }
}

#[axum::debug_handler]
async fn wake_host(Path(hostname): Path<String>, State(AppState{config_rx, ..}): State<AppState>) -> impl IntoResponse {
    let host = {
        let config = config_rx.borrow();
        let Some(host) = config.hosts.get(&hostname) else {
            return (StatusCode::NOT_FOUND, "Unknown host").into_response();
        };
        host.clone()
    };
    match send_magic_packet(&host.mac, "255.255.255.255") {
        Ok(_) => format!("Magic packet sent to {}", hostname).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response(),
    }
}

#[axum::debug_handler]
async fn shutdown_host(Path(hostname): Path<String>, State(AppState{config_rx, ..}): State<AppState>) -> impl IntoResponse {
    let host = {
        let config = config_rx.borrow();
        let Some(host) = config.hosts.get(&hostname) else {
            return (StatusCode::NOT_FOUND, "Unknown host").into_response();
        };
        host.clone()
    };
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let message = format!("{}|shutdown", timestamp);
    let signature = sign_hmac(&message, &host.shared_secret);
    let full_message = format!("{}|{}", message, signature);

    match send_shutdown(&host.ip, host.port, &full_message).await {
        Ok(resp) => resp.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
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
