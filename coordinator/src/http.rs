use axum::http::Request;
use axum::routing;
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
use shuthost_common::create_signed_message;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, watch};

use crate::assets::asset_routes;

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
        .nest("/api", api_routes())
        .nest("/download", get_download_router())
        .merge(asset_routes())
        .route("/ws", get(ws_handler))
        .with_state(app_state)
        .fallback(
            routing::any(|req: Request<axum::body::Body>| async move {
                tracing::warn!(
                    method = %req.method(),
                    uri = %req.uri(),
                    "Unhandled request"
                );
                Redirect::permanent("/")
            })
        );

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
            let shared_secret = host.shared_secret.clone();
            async move {
                let is_online = match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
                    Ok(Ok(mut stream)) => {
                        let signed_message = create_signed_message("status", &shared_secret);
                        // Send message
                        if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                            debug!("Failed to write to {}: {}", name, e);
                            return (name, false);
                        }
                        // Read response (optional, but let's check for a valid reply)
                        let mut buf = vec![0u8; 256];
                        match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                            Ok(Ok(n)) if n > 0 => {
                                let resp = String::from_utf8_lossy(&buf[..n]);
                                // Accept any non-error response as online
                                !resp.contains("ERROR")
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                };
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
