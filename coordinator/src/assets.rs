//! Static asset serving for the coordinator WebUI.
//!
//! Provides Axum routes to serve HTML, JS, CSS, images, and manifest.

use crate::http::AppState;
use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
};
use std::sync::OnceLock;

/// Returns the router handling core UI assets (HTML, manifest, favicon, SVGs).
pub fn asset_routes() -> Router<AppState> {
    Router::new()
    .route("/", get(serve_ui))
    .route("/manifest.json", get(serve_manifest))
    .route("/favicon.svg", get(serve_favicon))
    .route(
        "/architecture_simplified.svg",
        get(serve_architecture_simplified),
    )
    .route("/architecture.svg", get(serve_architecture_complete))
}
/// Serves the main HTML template, injecting dynamic content.
pub async fn serve_ui(State(AppState { config_path, .. }): State<AppState>) -> impl IntoResponse {
    static HTML_TEMPLATE: OnceLock<String> = OnceLock::new();
    
    let html = HTML_TEMPLATE
    .get_or_init(|| {
        include_str!("../assets/index.tmpl.html")
        .replace("{coordinator_config}", &config_path.to_string_lossy())
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
        .replace(
            "{ architecture_documentation }",
            include_str!("../assets/architecture.md"),
        )
        .replace(
            "{ client_install_requirements_gotchas }",
            include_str!("../assets/client_install_requirements_gotchas.md"),
        )
        .replace(
            "{ agent_install_requirements_gotchas }",
            include_str!("../assets/agent_install_requirements_gotchas.md"),
        )
        .replace("{version}", env!("CARGO_PKG_VERSION"))
        .replace(
            "/* {styles} */",
            include_str!("../assets/styles_output.css"),
        )
        .replace("{ js }", include_str!("../assets/app.js"))
    })
    .clone();
    
    Response::builder()
    .header("Content-Type", "text/html")
    .body(html.into_response())
    .unwrap()
}

/// Serves the manifest.json file for web app metadata.
pub async fn serve_manifest() -> impl IntoResponse {
    static MANIFEST: OnceLock<String> = OnceLock::new();
    
    let manifest = MANIFEST
    .get_or_init(|| {
        include_str!("../assets/manifest.json")
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
    })
    .clone();
    
    Response::builder()
    .header("Content-Type", "application/json")
    .body(manifest.into_response())
    .unwrap()
}

/// Serves the site favicon (SVG).
pub async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
    .header("Content-Type", "image/svg+xml")
    .body(include_bytes!("../assets/favicon.svg").into_response())
    .unwrap()
}

/// Serves simplified architecture SVG.
pub async fn serve_architecture_simplified() -> impl IntoResponse {
    Response::builder()
    .header("Content-Type", "image/svg+xml")
    .body(include_bytes!("../assets/architecture_simplified.svg").into_response())
    .unwrap()
}

/// Serves full architecture SVG.
pub async fn serve_architecture_complete() -> impl IntoResponse {
    Response::builder()
    .header("Content-Type", "image/svg+xml")
    .body(include_bytes!("../assets/architecture.svg").into_response())
    .unwrap()
}
