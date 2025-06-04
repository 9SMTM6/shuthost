use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use std::{net::IpAddr, time::Duration};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::{
    config::{ControllerConfig, load_coordinator_config, watch_config_file},
    routes::{LeaseMap, api_routes, get_download_router},
};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use clap::Parser;
use std::collections::HashMap;
use tokio::sync::{broadcast, watch};

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
    pub hoststatus_rx: watch::Receiver<Arc<HashMap<String, bool>>>,
    pub ws_tx: broadcast::Sender<String>,
    pub leases: LeaseMap,
}

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
    let (hoststatus_tx, hoststatus_rx) = watch::channel(initial_status);

    let (ws_tx, _) = broadcast::channel(32);

    {
        let config_rx = config_rx.clone();
        let hoststatus_tx = hoststatus_tx.clone();
        tokio::spawn(async move {
            poll_host_statuses(config_rx, hoststatus_tx).await;
        });
    }
    {
        let ws_tx = ws_tx.clone();
        let mut hoststatus_rx = hoststatus_rx.clone();
        tokio::spawn(async move {
            {
                let initial_status = hoststatus_tx.borrow().clone();
                let _ = ws_tx.send(serde_json::to_string(initial_status.as_ref()).unwrap());
            }
            loop {
                if let Ok(_) = hoststatus_rx.changed().await {
                    if let Ok(status_map) = serde_json::to_string(hoststatus_rx.borrow().as_ref()) {
                        let _ = ws_tx.send(status_map);
                    } else {
                        error!("Failed to serialize status map");
                    }
                } else {
                    warn!("Failed to receive change notification from is_on_tx");
                }
            }
        });
    }

    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(async move {
            // TODO: warn on changed port
            watch_config_file(path, config_tx).await;
        });
    }
    {
        let ws_tx = ws_tx.clone();
        let mut config_rx = config_rx.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(_) = config_rx.changed().await {
                    let _ = ws_tx.send("config_updated".to_string());
                    // if let Ok(config) = serde_json::to_string(config_rx.borrow().as_ref()) {
                    //     if let Err(e) = ws_tx.send(config) {
                    //         error!("Failed to send message on WebSocket: {}", e);
                    //     }
                    // } else {
                    //     error!("Failed to serialize status map");
                    // }
                } else {
                    warn!("WebSocket channel closed");
                    break;
                }
            }
        });
    }

    let app_state = AppState {
        config_rx,
        hoststatus_rx,
        ws_tx,
        config_path: config_path.to_path_buf(),
        leases: LeaseMap::default(),
    };

    let app = Router::new()
        .fallback(get(|| async { Redirect::permanent("/") }))
        .nest("/api", api_routes())
        .nest("/download", get_download_router())
        .route("/", get(serve_ui))
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
    hoststatus_tx: watch::Sender<Arc<HashMap<String, bool>>>,
) {
    loop {
        let config = config_rx.borrow().clone();

        let futures = config.nodes.iter().map(|(name, host)| {
            let addr = format!("{}:{}", host.ip, host.port);
            let name = name.clone();
            async move {
                let is_online = matches!(
                    timeout(Duration::from_millis(200), TcpStream::connect(&addr)).await,
                    Ok(Ok(_))
                );
                debug!("Polled {} at {} - online: {}", name, addr, is_online);
                (name, is_online)
            }
        });

        let results = futures::future::join_all(futures).await;
        let status_map: HashMap<_, _> = results.into_iter().collect();

        let is_new = {
            let old_status_map = hoststatus_tx.borrow();
            let old_status_map = old_status_map.as_ref();
            status_map != *old_status_map
        };
        if is_new {
            info!("Host status changed: {:?}", status_map);
            hoststatus_tx.send(Arc::new(status_map)).unwrap();
        } else {
            debug!("No change in host status");
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn serve_ui(State(AppState { config_path, .. }): State<AppState>) -> impl IntoResponse {
    let styles = include_str!("../assets/styles_output.css");
    let javascript = include_str!("../assets/app.js");

    let html = include_str!("../assets/index.tmpl.html")
        .replace("{coordinator_config}", &config_path.to_string_lossy())
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{version}", env!("CARGO_PKG_VERSION"))
        .replace("/* {styles} */", styles)
        .replace("{js}", javascript);

    Response::builder()
        .header("Content-Type", "text/html")
        .body(html.into_response())
        .unwrap()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(AppState {
        ws_tx,
        hoststatus_rx,
        ..
    }): State<AppState>,
) -> impl IntoResponse {
    let current_state = hoststatus_rx.borrow().clone();
    ws.on_upgrade(move |socket| handle_socket(socket, ws_tx.subscribe(), current_state))
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<String>,
    current_state: Arc<HashMap<String, bool>>,
) {
    tokio::spawn(async move {
        socket
            .send(
                serde_json::to_string(current_state.as_ref())
                    .unwrap()
                    .into(),
            )
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to send initial state: {}", e);
            });
        while let Ok(msg) = rx.recv().await {
            if socket.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
}

async fn serve_manifest() -> impl IntoResponse {
    let manifest = include_str!("../assets/manifest.json")
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"));

    Response::builder()
        .header("Content-Type", "application/json")
        .body(manifest.into_response())
        .unwrap()
}

async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "image/svg+xml")
        .body(include_bytes!("../assets/favicon.svg").into_response())
        .unwrap()
}
