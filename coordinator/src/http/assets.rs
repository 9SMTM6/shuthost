//! Static asset serving for the coordinator `WebUI`.
//!
//! Provides Axum routes to serve HTML, JS, CSS, images, and manifest.

use alloc::borrow;
use core::time::Duration;
use std::path;

use axum::{
    Router,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use axum_extra::{
    TypedHeader,
    headers::{CacheControl, ContentLength, ContentType},
};
use mime::{IMAGE_SVG, TEXT_CSS};
use serde::Serialize;

use crate::{app::AppState, http::EXPECTED_AUTH_EXCEPTIONS_VERSION, http::auth::Resolved};

#[expect(
    nonstandard_style,
    reason = "the functions should be const, in lack of that use a fn"
)]
fn IMMUTABLE_HEADER() -> TypedHeader<CacheControl> {
    TypedHeader(
        CacheControl::new()
            .with_immutable()
            .with_public()
            .with_max_age(Duration::from_hours(8760)),
    )
}

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
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            concat!("/manifest.", env!("ASSET_HASH_MANIFEST_JSON"), ".json"),
            get(serve_manifest),
        )
        .route(
            concat!("/styles.", env!("ASSET_HASH_STYLES_CSS"), ".css"),
            get(serve_styles),
        )
        .route(
            concat!("/app.", env!("ASSET_HASH_APP_JS"), ".js"),
            get(serve_app_js),
        )
        .route(
            "/favicon.svg",
            get(async || {
                Redirect::to(concat!("/favicon.", env!("ASSET_HASH_FAVICON_SVG"), ".svg"))
            }),
        )
        .route(
            concat!("/favicon.", env!("ASSET_HASH_FAVICON_SVG"), ".svg"),
            get(serve_favicon),
        )
        .route(
            concat!("/icons/icon-32.", env!("ASSET_HASH_ICON_32_PNG"), ".png"),
            get(serve_icon_32),
        )
        .route(
            concat!("/icons/icon-48.", env!("ASSET_HASH_ICON_48_PNG"), ".png"),
            get(serve_icon_48),
        )
        .route(
            concat!("/icons/icon-64.", env!("ASSET_HASH_ICON_64_PNG"), ".png"),
            get(serve_icon_64),
        )
        .route(
            concat!("/icons/icon-128.", env!("ASSET_HASH_ICON_128_PNG"), ".png"),
            get(serve_icon_128),
        )
        .route(
            concat!("/icons/icon-180.", env!("ASSET_HASH_ICON_180_PNG"), ".png"),
            get(serve_icon_180),
        )
        .route(
            concat!("/icons/icon-192.", env!("ASSET_HASH_ICON_192_PNG"), ".png"),
            get(serve_icon_192),
        )
        .route(
            concat!("/icons/icon-512.", env!("ASSET_HASH_ICON_512_PNG"), ".png"),
            get(serve_icon_512),
        )
        .route("/sw.js", get(serve_service_worker))
}

/// Macro to define a static SVG download handler using `include_bytes`!
macro_rules! static_svg_download_handler {
    (fn $name:ident, file=$file:expr) => {
        #[axum::debug_handler]
        async fn $name() -> impl IntoResponse {
            const SVG: &'static str = include_utf8_asset!($file);
            (
                TypedHeader(ContentType::from(IMAGE_SVG)),
                IMMUTABLE_HEADER(),
                TypedHeader(ContentLength(SVG.len() as u64)),
                SVG,
            )
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
            (
                TypedHeader(ContentType::png()),
                IMMUTABLE_HEADER(),
                TypedHeader(ContentLength(DATA.len() as u64)),
                DATA,
            )
        }
    };
}

/// HTML rendering mode for the UI template
pub(crate) enum UiMode<'params> {
    Normal {
        config_path: &'params path::Path,
        auth_warning: bool,
        auth_mode: &'static str,
        broadcast_port: u16,
        db_enabled: bool,
    },
    Demo {
        subpath: &'params str,
    },
}

/// Holds static server data injected into the UI; this should not contain sensitive data.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiServerData<'strings> {
    config_path: borrow::Cow<'strings, str>,
    auth_warning: bool,
    /// Demo mode signal: `Some` means demo mode, `None` means normal mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    demo_subpath: Option<&'strings str>,
    auth_mode: &'strings str,
    broadcast_port: u16,
    db_enabled: bool,
}

/// Renders the main HTML template, injecting a JSON data island with all
/// server-side values that the `SolidJS` app needs on startup.
pub(crate) fn render_ui_html(mode: &UiMode<'_>) -> String {
    let server_data = match *mode {
        UiMode::Normal {
            config_path,
            auth_warning,
            auth_mode,
            broadcast_port,
            db_enabled,
        } => UiServerData {
            config_path: config_path.to_string_lossy(),
            auth_warning,
            demo_subpath: None,
            auth_mode,
            broadcast_port,
            db_enabled,
        },
        UiMode::Demo { subpath } => UiServerData {
            config_path: borrow::Cow::Borrowed("/this/is/a/demo.toml"),
            auth_warning: false,
            demo_subpath: Some(subpath),
            auth_mode: "disabled",
            broadcast_port: shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT,
            db_enabled: true,
        },
    };

    let server_data = serde_json::to_string(&server_data)
        .expect("UiServerData serialization should not fail")
        .replace("</", r"<\/");

    include_utf8_asset!("generated/index.html").replace("{ server_data }", &server_data)
}

/// Serves the main HTML template, injecting dynamic content.
pub(crate) fn serve_ui(
    AppState {
        config_path,
        auth,
        config_rx,
        db_pool,
        ..
    }: AppState,
) -> Response {
    type A = Resolved;

    // Show auth warning when auth is disabled, or when External auth is
    // configured but its exceptions_version doesn't match the expected value.
    let auth_warning = matches!(&auth.mode, A::Disabled)
        || matches!(
            &auth.mode,
            A::External { exceptions_version } if *exceptions_version != EXPECTED_AUTH_EXCEPTIONS_VERSION
        );

    let auth_mode = auth.mode.auth_mode_str();

    let broadcast_port = config_rx.borrow().server.broadcast_port;

    (
        TypedHeader(ContentType::html()),
        render_ui_html(&UiMode::Normal {
            config_path: &config_path,
            auth_warning,
            auth_mode,
            broadcast_port,
            db_enabled: db_pool.is_some(),
        }),
    )
        .into_response()
}

/// Serves the compiled JavaScript bundle for the SPA.
#[axum::debug_handler]
async fn serve_app_js() -> impl IntoResponse {
    const JS: &str = include_utf8_asset!("generated/app.js");
    (
        TypedHeader(ContentType::from(mime::TEXT_JAVASCRIPT)),
        IMMUTABLE_HEADER(),
        TypedHeader(ContentLength(JS.len() as u64)),
        JS,
    )
}

/// Serves the manifest.json file for web app metadata.
#[axum::debug_handler]
pub(crate) async fn serve_manifest() -> impl IntoResponse {
    (
        TypedHeader(ContentType::json()),
        IMMUTABLE_HEADER(),
        include_utf8_asset!("generated/manifest.json"),
    )
}

/// Serves the compiled stylesheet for the UI.
#[axum::debug_handler]
pub(crate) async fn serve_styles() -> impl IntoResponse {
    (
        TypedHeader(ContentType::from(TEXT_CSS)),
        IMMUTABLE_HEADER(),
        include_utf8_asset!("generated/app.css"),
    )
}

static_svg_download_handler!(fn serve_favicon, file = "favicon.svg");

// Binary icon handlers (generated in build.rs into frontend/assets/generated/icons)
static_png_download_handler!(fn serve_icon_32, file = "icon-32.png");
static_png_download_handler!(fn serve_icon_48, file = "icon-48.png");
static_png_download_handler!(fn serve_icon_64, file = "icon-64.png");
static_png_download_handler!(fn serve_icon_128, file = "icon-128.png");
static_png_download_handler!(fn serve_icon_180, file = "icon-180.png");
static_png_download_handler!(fn serve_icon_192, file = "icon-192.png");
static_png_download_handler!(fn serve_icon_512, file = "icon-512.png");

/// Serves the service worker script without caching so browsers always pick up updates.
#[axum::debug_handler]
async fn serve_service_worker() -> impl IntoResponse {
    const SW: &str = include_utf8_asset!("generated/sw.js");
    (
        TypedHeader(ContentType::from(mime::TEXT_JAVASCRIPT)),
        TypedHeader(CacheControl::new().with_no_store()),
        TypedHeader(ContentLength(SW.len() as u64)),
        SW,
    )
}
