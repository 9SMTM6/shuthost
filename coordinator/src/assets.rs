//! Static asset serving for the coordinator WebUI.
//!
//! Provides Axum routes to serve HTML, JS, CSS, images, and manifest.

use crate::auth::{AuthResolved, EXPECTED_EXCEPTIONS_VERSION};
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
        .route("/styles.css", get(serve_styles))
        .route("/favicon.svg", get(serve_favicon))
        .route(
            "/architecture_simplified.svg",
            get(serve_architecture_simplified),
        )
        .route("/architecture.svg", get(serve_architecture_complete))
}
/// HTML rendering mode for the UI template
pub enum UiMode<'a> {
    Normal {
        config_path: &'a std::path::Path,
        show_logout: bool,
    },
    Demo,
}

/// Renders the main HTML template, injecting dynamic content and demo disclaimer if needed.
pub fn render_ui_html(mode: &UiMode<'_>, maybe_external_auth_config: &str) -> String {
    let header_tabs = include_str!("../assets/partials/header_tabs.tmpl.html");
    let maybe_logout = if matches!(
        *mode,
        UiMode::Normal {
            show_logout: true,
            ..
        }
    ) {
        include_str!("../assets/partials/logout_form.tmpl.html")
    } else {
        ""
    };
    let maybe_demo_disclaimer = if matches!(*mode, UiMode::Demo) {
        include_str!("../assets/partials/demo_disclaimer.tmpl.html")
    } else {
        ""
    };
    let config_path = match *mode {
        UiMode::Normal { config_path, .. } => config_path.to_string_lossy().to_string(),
        UiMode::Demo => "/this/is/a/demo.toml".to_string(),
    };

    let header_tpl = include_str!("../assets/partials/header.tmpl.html");
    let footer_tpl = include_str!("../assets/partials/footer.tmpl.html");

    include_str!("../assets/index.tmpl.html")
        .replace("{ html_head }", include_str!("../assets/partials/html_head.tmlp.html"))
        .replace("{ title }", "ShutHost Coordinator")
        .replace("{ coordinator_config }", &config_path)
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace(
            "{ architecture_documentation }",
            include_str!("../assets/architecture.md"),
        )
        .replace("{ maybe_external_auth_config }", maybe_external_auth_config)
        .replace(
            "{ client_install_requirements_gotchas }",
            include_str!("../assets/client_install_requirements_gotchas.md"),
        )
        .replace(
            "{ agent_install_requirements_gotchas }",
            include_str!("../assets/agent_install_requirements_gotchas.md"),
        )
        .replace("{ js }", include_str!("../assets/app.js"))
        .replace("{ header }", header_tpl)
        .replace("{ footer }", footer_tpl)
        .replace("{ version }", env!("CARGO_PKG_VERSION"))
        .replace("{ maybe_logout }", maybe_logout)
        .replace("{ maybe_demo_disclaimer }", maybe_demo_disclaimer)
        .replace("{ maybe_tabs }", header_tabs)
        .replace("{ maybe_tabs }", header_tabs)
        .replace("{ maybe_logout }", maybe_logout)
}

/// Serves the main HTML template, injecting dynamic content.
pub async fn serve_ui(
    State(AppState {
        config_path, auth, ..
    }): State<AppState>,
) -> impl IntoResponse {
    static HTML_TEMPLATE: OnceLock<String> = OnceLock::new();
    let show_logout = !matches!(auth.mode, AuthResolved::Disabled);
    let html = HTML_TEMPLATE
        .get_or_init(|| {
            // Determine whether to include the external auth config warning. If Auth is
            // Disabled we must show it. If Auth::External is configured but its
            // exceptions_version doesn't match the current expected version, show it.
            type A = AuthResolved;
            let maybe_external_auth_config = match &auth.mode {
                &A::Token { .. }
                | &A::Oidc { .. }
                | &A::External {
                    exceptions_version: EXPECTED_EXCEPTIONS_VERSION,
                } => "",
                &A::Disabled | &A::External { .. } => {
                    include_str!("../assets/partials/maybe_external_auth_config.tmpl.html")
                }
            };

            render_ui_html(
                &UiMode::Normal {
                    config_path: &config_path,
                    show_logout,
                },
                maybe_external_auth_config,
            )
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
                .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        })
        .clone();

    Response::builder()
        .header("Content-Type", "application/json")
        .body(manifest.into_response())
        .unwrap()
}

/// Serves the compiled stylesheet for the UI.
pub async fn serve_styles() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "text/css")
        .body(include_str!("../assets/styles_output.css").into_response())
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
