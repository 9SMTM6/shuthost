//! Static asset serving for the coordinator WebUI.
//!
//! Provides Axum routes to serve HTML, JS, CSS, images, and manifest.

use std::sync::OnceLock;

use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
};

use crate::{
    auth::{EXPECTED_EXCEPTIONS_VERSION, Resolved},
    http::AppState,
};

#[macro_export]
macro_rules! include_utf8_asset {
    ($asset_path:expr) => {
        include_str!(concat!(
            env!("WORKSPACE_ROOT"),
            "frontend/assets/",
            $asset_path
        ))
    };
}

/// Returns the router handling core UI assets (manifest, favicon, SVGs) - except index.html.
pub fn asset_routes() -> Router<AppState> {
    Router::new()
        .route("/manifest.json", get(serve_manifest))
        .route("/styles.css", get(serve_styles))
        .route("/favicon.svg", get(serve_favicon))
        .route("/icons/icon-32.png", get(serve_icon_32))
        .route("/icons/icon-48.png", get(serve_icon_48))
        .route("/icons/icon-64.png", get(serve_icon_64))
        .route("/icons/icon-128.png", get(serve_icon_128))
        .route("/icons/icon-180.png", get(serve_icon_180))
        .route("/icons/icon-192.png", get(serve_icon_192))
        .route("/icons/icon-512.png", get(serve_icon_512))
        .route(
            "/architecture_simplified.svg",
            get(serve_architecture_simplified),
        )
        .route("/architecture.svg", get(serve_architecture_complete))
}

/// Macro to define a static SVG download handler using include_bytes!
macro_rules! static_svg_download_handler {
    (fn $name:ident, file=$file:expr) => {
        async fn $name() -> impl IntoResponse {
            const SVG: &'static str = include_utf8_asset!($file);
            Response::builder()
                .header("Content-Type", "image/svg+xml")
                .header("Content-Length", SVG.len().to_string())
                .body(SVG.into_response())
                .unwrap()
        }
    };
}

/// Macro to define a static png download handler.
macro_rules! static_png_download_handler {
    (fn $name:ident, file=$file:expr) => {
        async fn $name() -> impl IntoResponse {
            const DATA: &[u8] =
                include_bytes!(concat!(env!("WORKSPACE_ROOT"), "frontend/assets/", $file));
            Response::builder()
                .header("Content-Type", "image/png")
                .header("Content-Length", DATA.len().to_string())
                .body(DATA.into_response())
                .unwrap()
        }
    };
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
    let maybe_logout = if matches!(
        *mode,
        UiMode::Normal {
            show_logout: true,
            ..
        }
    ) {
        include_utf8_asset!("partials/logout_form.tmpl.html")
    } else {
        ""
    };
    let maybe_demo_disclaimer = if matches!(*mode, UiMode::Demo) {
        include_utf8_asset!("partials/demo_disclaimer.tmpl.html")
    } else {
        ""
    };
    let config_path = match *mode {
        UiMode::Normal { config_path, .. } => config_path.to_string_lossy().to_string(),
        UiMode::Demo => "/this/is/a/demo.toml".to_string(),
    };

    let header_tpl = include_utf8_asset!("partials/header.tmpl.html");
    let footer_tpl = include_utf8_asset!("partials/footer.tmpl.html");

    include_utf8_asset!("/index.tmpl.html")
        .replace(
            "{ html_head }",
            include_utf8_asset!("partials/html_head.tmlp.html"),
        )
        .replace("{ title }", "ShutHost Coordinator")
        .replace("{ coordinator_config }", &config_path)
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace(
            "{ architecture_documentation }",
            include_utf8_asset!("partials/architecture.tmpl.html"),
        )
        .replace("{ maybe_external_auth_config }", maybe_external_auth_config)
        .replace(
            "{ client_install_requirements_gotchas }",
            include_utf8_asset!("client_install_requirements_gotchas.md"),
        )
        .replace(
            "{ agent_install_requirements_gotchas }",
            include_utf8_asset!("agent_install_requirements_gotchas.md"),
        )
        .replace("{ js }", include_utf8_asset!("app.js"))
        .replace("{ header }", header_tpl)
        .replace("{ footer }", footer_tpl)
        .replace("{ version }", env!("CARGO_PKG_VERSION"))
        .replace("{ maybe_logout }", maybe_logout)
        .replace("{ maybe_demo_disclaimer }", maybe_demo_disclaimer)
}

/// Serves the main HTML template, injecting dynamic content.
///
/// # Panics
///
/// Panics if the response builder fails to build the response.
pub async fn serve_ui(
    State(AppState {
        config_path, auth, ..
    }): State<AppState>,
) -> impl IntoResponse {
    static HTML_TEMPLATE: OnceLock<String> = OnceLock::new();
    let show_logout = !matches!(auth.mode, Resolved::Disabled | Resolved::External { .. });
    let html = HTML_TEMPLATE
        .get_or_init(|| {
            // Determine whether to include the external auth config warning. If Auth is
            // Disabled we must show it. If Auth::External is configured but its
            // exceptions_version doesn't match the current expected version, show it.
            type A = Resolved;
            let maybe_external_auth_config = match &auth.mode {
                &A::Token { .. }
                | &A::Oidc { .. }
                | &A::External {
                    exceptions_version: EXPECTED_EXCEPTIONS_VERSION,
                } => "",
                &A::Disabled | &A::External { .. } => {
                    include_utf8_asset!("partials/maybe_external_auth_config.tmpl.html")
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
///
/// # Panics
///
/// Panics if the response builder fails to build the response.
pub async fn serve_manifest() -> impl IntoResponse {
    static MANIFEST: OnceLock<String> = OnceLock::new();

    let manifest = MANIFEST
        .get_or_init(|| {
            include_utf8_asset!("manifest.tmpl.json")
                .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        })
        .clone();

    Response::builder()
        .header("Content-Type", "application/json")
        .body(manifest.into_response())
        .unwrap()
}

/// Serves the compiled stylesheet for the UI.
///
/// # Panics
///
/// Panics if the response builder fails to build the response.
pub async fn serve_styles() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "text/css")
        .body(include_utf8_asset!("styles.css").into_response())
        .unwrap()
}

static_svg_download_handler!(fn serve_favicon, file = "favicon.svg");
static_svg_download_handler!(fn serve_architecture_simplified, file = "architecture_simplified.svg");
static_svg_download_handler!(fn serve_architecture_complete, file = "architecture.svg");

// Binary icon handlers (generated in build.rs into frontend/assets/icons)
static_png_download_handler!(fn serve_icon_32, file = "icons/icon-32.png");
static_png_download_handler!(fn serve_icon_48, file = "icons/icon-48.png");
static_png_download_handler!(fn serve_icon_64, file = "icons/icon-64.png");
static_png_download_handler!(fn serve_icon_128, file = "icons/icon-128.png");
static_png_download_handler!(fn serve_icon_180, file = "icons/icon-180.png");
static_png_download_handler!(fn serve_icon_192, file = "icons/icon-192.png");
static_png_download_handler!(fn serve_icon_512, file = "icons/icon-512.png");
