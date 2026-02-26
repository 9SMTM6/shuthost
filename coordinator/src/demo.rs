//! Demo service implementation for serving static assets.
//!
//! This module provides a minimal demo mode that serves only static assets
//! without any backend state or functionality.

use alloc::sync::Arc;
use std::{collections::HashMap, path};

use axum::{Router, extract::State, http::Response, routing};
use tokio::{
    net::TcpListener,
    sync::{broadcast, watch},
};
use tracing::info;

use crate::{
    app::{startup::shutdown_signal, state::AppState},
    config::{AuthConfig, ControllerConfig},
    http::auth,
    http::{
        assets::{self, UiMode, render_ui_html},
        download,
        m2m::LeaseMap,
    },
};

/// Run the demo service on the specified port and bind address.
///
/// # Panics
///
/// Panics if the TCP listener cannot be bound to the specified address.
pub(crate) async fn run_demo_service(port: u16, bind: &str, subpath: &str) {
    let addr = format!("{bind}:{port}");
    info!("Starting demo service on http://{}", addr);

    // Custom asset route for demo mode: inject disclaimer into HTML
    let serve_demo_ui = {
        let subpath = subpath.to_string();
        move |State(_): State<AppState>| {
            let subpath = subpath.clone();
            async move {
                let html = render_ui_html(&UiMode::Demo { subpath: &subpath });
                Response::builder()
                    .header("Content-Type", "text/html")
                    .body(html)
                    .expect("failed to build HTTP response")
            }
        }
    };

    let (hoststatus_tx, hoststatus_rx) = watch::channel(Arc::new(HashMap::new()));

    let app_state = AppState {
        config_path: path::PathBuf::from("demo"),
        config_rx: watch::channel(Arc::new(ControllerConfig::default())).1,
        hoststatus_rx,
        hoststatus_tx,
        ws_tx: broadcast::channel(1).0,
        leases: LeaseMap::default(),
        auth: Arc::new(
            auth::Runtime::from_config(&AuthConfig::default(), None)
                .await
                .expect("failed to initialize auth runtime"),
        ),
        tls_enabled: false,
        db_pool: None,
    };

    let app = Router::new()
        .route("/", routing::get(serve_demo_ui))
        .merge(assets::routes())
        .nest("/download", download::routes())
        .with_state(app_state);

    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Demo server failed");
}
