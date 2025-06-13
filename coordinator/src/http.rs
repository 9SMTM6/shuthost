use axum::{Router, response::Redirect, routing::get};
use std::{net::IpAddr, time::Duration};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, info};

use crate::{
    config::{ControllerConfig, load_coordinator_config, watch_config_file},
    routes::{LeaseMap, api_routes, get_download_router},
    websocket::{WsMessage, ws_handler},
};
use clap::Parser;
use std::collections::HashMap;
use tokio::sync::{broadcast, watch};

use crate::assets::{serve_favicon, serve_manifest, serve_ui};

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
    pub ws_tx: broadcast::Sender<WsMessage>,
    pub leases: LeaseMap,
}

pub async fn start_http_server(
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting HTTP server...");

    let initial_config = Arc::new(load_coordinator_config(config_path).await?);
    let listen_port = initial_config.server.port;

    let listen_ip: IpAddr = initial_config.server.bind.parse()?;

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
            while hoststatus_rx.changed().await.is_ok() {
                let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                let _ = ws_tx.send(msg);
            }
        });
    }

    {
        let ws_tx = ws_tx.clone();
        let mut config_rx = config_rx.clone();
        tokio::spawn(async move {
            while config_rx.changed().await.is_ok() {
                let config = config_rx.borrow();
                let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
                let clients = config.clients.keys().cloned().collect::<Vec<_>>();
                let msg = WsMessage::ConfigChanged { hosts, clients };
                let _ = ws_tx.send(msg);
            }
        });
    }

    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(async move {
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

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn poll_host_statuses(
    config_rx: watch::Receiver<Arc<ControllerConfig>>,
    hoststatus_tx: watch::Sender<Arc<HashMap<String, bool>>>,
) {
    loop {
        let config = config_rx.borrow().clone();

        let futures = config.hosts.iter().map(|(name, host)| {
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
