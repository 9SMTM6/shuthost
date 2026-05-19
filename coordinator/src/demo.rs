//! Demo service implementation for serving static assets.
//!
//! This module provides a minimal demo mode that serves only static assets
//! without any backend state or functionality.

use alloc::sync::Arc;
use std::{collections::HashMap, path};

use axum::{http::Response, response::IntoResponse as _};
use tokio::{
    net::TcpListener,
    sync::{broadcast, watch},
};
use tracing::info;

use crate::{
    app::{
        AppState, HostActorHandle, LeaseMapRaw, LeaseStore, OperationFailureStore, RwMap,
        shutdown_signal,
    },
    config::{AuthConfig, ControllerConfig, RuntimeConfig},
    http::{
        assets::{UiMode, render_ui_html},
        auth,
        server::router::create_app_router,
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
        move |_: AppState| {
            let html = render_ui_html(&UiMode::Demo { subpath: &subpath });
            Response::builder()
                .header("Content-Type", "text/html")
                .body(html)
                .expect("failed to build HTTP response")
                .into_response()
        }
    };

    let hoststatus = HostActorHandle::spawn(HashMap::new());

    let app_state = AppState {
        config_path: path::PathBuf::from("demo"),
        config_rx: watch::channel(Arc::new(ControllerConfig::default())).1,
        host_actor: hoststatus,
        ws_tx: broadcast::channel(1).0,
        leases: LeaseStore::new(LeaseMapRaw::default()).0,
        host_overrides: RwMap::default(),
        host_install_info: RwMap::default(),
        auth: Arc::new(
            auth::Runtime::from_config(&AuthConfig::default(), None)
                .await
                .expect("failed to initialize auth runtime"),
        ),
        tls_enabled: false,
        runtime: RuntimeConfig::default(),
        db_pool: None,
        vapid_key: None,
        operation_failures: OperationFailureStore::new(HashMap::new()).0,
        online_since: RwMap::default(),
    };

    let app = create_app_router(&app_state.auth, serve_demo_ui).with_state(app_state);

    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Demo server failed");
}
