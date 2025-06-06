use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Redirect},
    routing::get,
};
use std::{net::IpAddr, time::Duration};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::{
    config::{ControllerConfig, load_coordinator_config, watch_config_file},
    routes::{LeaseMap, api_routes, get_download_router},
};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use clap::Parser;
use std::collections::HashMap;
use tokio::sync::{broadcast, watch};
use serde::{Serialize, Deserialize};

use crate::assets::{serve_ui, serve_manifest, serve_favicon};

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
    pub ws_tx: broadcast::Sender<WsMessage>,  // Changed from String to WsMessage
    pub leases: LeaseMap,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    HostStatus(HashMap<String, bool>),
    UpdateNodes(Vec<String>),
    Initial {
        nodes: Vec<String>,
        status: HashMap<String, bool>,
    },
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
                let initial_status = hoststatus_rx.borrow().clone();
                let msg = WsMessage::HostStatus(initial_status.as_ref().clone());
                let _ = ws_tx.send(msg);  // Remove json conversion
            }
            loop {
                if hoststatus_rx.changed().await.is_ok() {
                    let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                    let _ = ws_tx.send(msg);  // Remove json conversion
                }
            }
        });
    }
    {
        let ws_tx = ws_tx.clone();
        let mut config_rx = config_rx.clone();
        tokio::spawn(async move {
            loop {
                if config_rx.changed().await.is_ok() {
                    let config = config_rx.borrow();
                    let nodes = config.nodes.keys().cloned().collect::<Vec<_>>();
                    let msg = WsMessage::UpdateNodes(nodes);
                    let _ = ws_tx.send(msg);  // Remove json conversion
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(AppState {
        ws_tx,
        hoststatus_rx,
        config_rx,
        ..
    }): State<AppState>,
) -> impl IntoResponse {
    let current_state = hoststatus_rx.borrow().clone();
    ws.on_upgrade(move |socket| handle_socket(
        socket,
        ws_tx.subscribe(),
        current_state,
        config_rx
    ))
}

async fn send_ws_message(socket: &mut WebSocket, msg: &WsMessage) -> Result<(), axum::Error> {
    match serde_json::to_string(msg) {
        Ok(json) => socket.send(Message::Text(json.into())).await,
        Err(e) => {
            warn!("Failed to serialize websocket message: {}", e);
            Err(axum::Error::new(e))
        }
    }
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<WsMessage>,
    current_state: Arc<HashMap<String, bool>>,
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
) {
    tokio::spawn(async move {
        // Send initial combined state
        let nodes = config_rx.borrow().nodes.keys().cloned().collect();
        let initial_msg = WsMessage::Initial {
            nodes,
            status: current_state.as_ref().clone(),
        };
        
        if let Err(e) = send_ws_message(&mut socket, &initial_msg).await {
            warn!("Failed to send initial state: {}", e);
            return;
        }

        // Handle broadcast messages
        while let Ok(msg) = rx.recv().await {
            if let Err(e) = send_ws_message(&mut socket, &msg).await {
                warn!("Failed to send message, closing connection: {}", e);
                break;
            }
        }
    });
}
