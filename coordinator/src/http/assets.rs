//! Static asset serving for the coordinator WebUI.
//!
//! Provides Axum routes to serve HTML, JS, CSS, images, and manifest.

use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
};

use crate::{auth::Resolved, http::AppState};

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
pub fn routes() -> Router<AppState> {
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
        #[axum::debug_handler]
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
        #[axum::debug_handler]
        async fn $name() -> impl IntoResponse {
            const DATA: &[u8] = include_bytes!(concat!(
                env!("WORKSPACE_ROOT"),
                "frontend/assets/generated/icons/",
                $file
            ));
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
pub fn render_ui_html(mode: &UiMode<'_>) -> String {
    let maybe_logout = if matches!(
        *mode,
        UiMode::Normal {
            show_logout: true,
            ..
        }
    ) {
        include_utf8_asset!("partials/logout_form.html")
    } else {
        ""
    };
    let maybe_demo_disclaimer = if matches!(*mode, UiMode::Demo) {
        include_utf8_asset!("partials/demo_disclaimer.html")
    } else {
        ""
    };
    let config_path = match *mode {
        UiMode::Normal { config_path, .. } => config_path.to_string_lossy().to_string(),
        UiMode::Demo => "/this/is/a/demo.toml".to_string(),
    };

    include_utf8_asset!("generated/index.html")
        .replace("{ coordinator_config }", &config_path)
        .replace("{ maybe_logout }", maybe_logout)
        .replace("{ maybe_demo_disclaimer }", maybe_demo_disclaimer)
}

/// Serves the main HTML template, injecting dynamic content.
///
/// # Panics
///
/// Panics if the response builder fails to build the response.
#[axum::debug_handler]
pub async fn serve_ui(
    State(AppState {
        config_path, auth, ..
    }): State<AppState>,
) -> impl IntoResponse {
    let show_logout = !matches!(auth.mode, Resolved::Disabled | Resolved::External { .. });

    Response::builder()
        .header("Content-Type", "text/html")
        .body(
            render_ui_html(&UiMode::Normal {
                config_path: &config_path,
                show_logout,
            })
            .into_response(),
        )
        .unwrap()
}

/// Serves the manifest.json file for web app metadata.
///
/// # Panics
///
/// Panics if the response builder fails to build the response.
#[axum::debug_handler]
pub async fn serve_manifest() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "application/json")
        .body(include_utf8_asset!("generated/manifest.json").into_response())
        .unwrap()
}

/// Serves the compiled stylesheet for the UI.
///
/// # Panics
///
/// Panics if the response builder fails to build the response.
#[axum::debug_handler]
pub async fn serve_styles() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "text/css")
        .body(include_utf8_asset!("generated/styles.css").into_response())
        .unwrap()
}

static_svg_download_handler!(fn serve_favicon, file = "favicon.svg");
static_svg_download_handler!(fn serve_architecture_simplified, file = "generated/architecture_simplified.svg");
static_svg_download_handler!(fn serve_architecture_complete, file = "generated/architecture.svg");

// Binary icon handlers (generated in build.rs into frontend/assets/generated/icons)
static_png_download_handler!(fn serve_icon_32, file = "icon-32.png");
static_png_download_handler!(fn serve_icon_48, file = "icon-48.png");
static_png_download_handler!(fn serve_icon_64, file = "icon-64.png");
static_png_download_handler!(fn serve_icon_128, file = "icon-128.png");
static_png_download_handler!(fn serve_icon_180, file = "icon-180.png");
static_png_download_handler!(fn serve_icon_192, file = "icon-192.png");
static_png_download_handler!(fn serve_icon_512, file = "icon-512.png");
