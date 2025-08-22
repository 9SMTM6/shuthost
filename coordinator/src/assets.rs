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
        .route("/manifest.json", get(serve_manifest))
        .route("/favicon.svg", get(serve_favicon))
        .route(
            "/architecture_simplified.svg",
            get(serve_architecture_simplified),
        )
        .route("/architecture.svg", get(serve_architecture_complete))
}
/// HTML rendering mode for the UI template
pub enum UiMode<'a> {
    Normal { config_path: &'a std::path::Path },
    Demo,
}

/// Renders the main HTML template, injecting dynamic content and demo disclaimer if needed.
pub fn render_ui_html(mode: &UiMode<'_>) -> String {
    let (config_path, demo_disclaimer) = match *mode {
        UiMode::Normal { config_path } => (
            config_path.to_string_lossy().to_string(),
            "".to_string(),
        ),
        UiMode::Demo => (
            "/this/is/a/demo.toml".to_string(),
            "<div id=\"demo-mode-disclaimer\" style=\"background:#ffc; color:#222; padding:1em; text-align:center; font-weight:bold;\">Demo Mode: Static UI with simulated interactions only</div>".to_string(),
        ),
    };

    include_str!("../assets/index.tmpl.html")
        .replace("{coordinator_config}", &config_path)
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
        .replace("{demo_disclaimer}", &demo_disclaimer)
}

/// Serves the main HTML template, injecting dynamic content.
pub async fn serve_ui(State(AppState { config_path, .. }): State<AppState>) -> impl IntoResponse {
    static HTML_TEMPLATE: OnceLock<String> = OnceLock::new();
    let html = HTML_TEMPLATE
        .get_or_init(|| {
            render_ui_html(&UiMode::Normal {
                config_path: &config_path,
            })
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
