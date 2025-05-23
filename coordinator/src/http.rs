use axum::{
    extract::State, response::{IntoResponse, Redirect, Response}, routing::get, Router
};
use std::{net::IpAddr, time::Duration};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info};

use crate::{
    config::{load_coordinator_config, watch_config_file, ControllerConfig},
    routes::{api_routes, get_download_router, LeaseMap},
};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use clap::Parser;
use std::collections::HashMap;
use tokio::sync::broadcast;

#[derive(Debug, Parser)]
pub struct ServiceArgs {
    #[arg(
        long = "config",
        env = "SHUTHOST_CONTROLLER_CONFIG_PATH",
        default_value = "shuthost_coordinator.toml"
    )]
    pub config: String,
}

#[derive(Clone)]
pub struct AppState {
    pub config_path: std::path::PathBuf,
    pub config_rx: watch::Receiver<Arc<ControllerConfig>>,
    pub is_on_rx: watch::Receiver<Arc<HashMap<String, bool>>>,
    pub ws_tx: broadcast::Sender<String>,
    pub leases: LeaseMap,
}

use tokio::sync::watch;

pub async fn start_http_server(config_path: &std::path::Path) {
    info!("Starting HTTP server...");

    let initial_config = Arc::new(
        load_coordinator_config(config_path)
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
        leases: LeaseMap::default(),
    };

    let app = Router::new()
        .nest("/api", api_routes())
        .nest("/download", get_download_router())
        .route("/", get(|| async {Redirect::permanent("/index.html")}))
        .route("/index.html", get(serve_ui))
        .route("/ws", get(ws_handler))
        .route("/manifest.json", get(serve_manifest))
        .route("/favicon.svg", get(serve_favicon))
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

async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    is_on_tx: watch::Sender<Arc<HashMap<String, bool>>>,
    ws_tx: broadcast::Sender<String>,
) {
    loop {
        let config = config_rx.borrow().clone();
        let mut status_map = HashMap::new();

        for (name, host) in &config.nodes {
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

async fn serve_ui(State(AppState { config_path, .. }): State<AppState>) -> impl IntoResponse {
    let html = include_str!("../index.html")
        .replace("{coordinator_config}", &config_path.to_string_lossy())
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"));
    
    Response::builder()
        .header("Content-Type", "text/html")
        .body(html.into_response())
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

async fn serve_manifest() -> impl IntoResponse {
    let manifest = include_str!("../manifest.json")
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"));

    Response::builder()
        .header("Content-Type", "application/json")
        .body(manifest.into_response())
        .unwrap()
}

async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "image/svg+xml")
        .body(include_bytes!("../favicon.svg").into_response())
        .unwrap()
}
