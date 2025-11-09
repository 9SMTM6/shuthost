//! Demo service implementation for serving static assets.
//!
//! This module provides a minimal demo mode that serves only static assets
//! without any backend state or functionality.

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use axum::{Router, extract::State, http::Response, response::IntoResponse};
use tokio::{
    net::TcpListener,
    sync::{broadcast, watch},
};
use tracing::info;

use crate::{
    config::{AuthConfig, ControllerConfig},
    http::{
        AppState,
        assets::{UiMode, render_ui_html},
        download,
        m2m::{self, LeaseMap},
    },
};

/// Run the demo service on the specified port and bind address.
///
/// # Panics
///
/// Panics if the TCP listener cannot be bound to the specified address.
pub async fn run_demo_service(port: u16, bind: &str) {
    let addr = format!("{}:{}", bind, port);
    info!("Starting demo service on http://{}", addr);

    // Custom asset route for demo mode: inject disclaimer into HTML
    async fn serve_demo_ui(State(_): State<AppState>) -> impl IntoResponse {
        static HTML_TEMPLATE: OnceLock<String> = OnceLock::new();
        let html = HTML_TEMPLATE
            .get_or_init(|| render_ui_html(&UiMode::Demo, ""))
            .clone();
        Response::builder()
            .header("Content-Type", "text/html")
            .body(html)
            .unwrap()
    }

    let (hoststatus_tx, hoststatus_rx) = watch::channel(Arc::new(HashMap::new()));

    let app_state = AppState {
        config_path: std::path::PathBuf::from("demo"),
        config_rx: watch::channel(Arc::new(ControllerConfig::default())).1,
        hoststatus_rx,
        hoststatus_tx,
        ws_tx: broadcast::channel(1).0,
        leases: LeaseMap::default(),
        auth: std::sync::Arc::new(
            crate::auth::Runtime::from_config(&AuthConfig::default(), None)
                .await
                .unwrap(),
        ),
        tls_enabled: false,
        db_pool: None,
    };

    let app = Router::new()
        .route("/", axum::routing::get(serve_demo_ui))
        .merge(m2m::routes())
        .nest("/download", download::routes())
        .with_state(app_state);

    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Demo server failed");
}
