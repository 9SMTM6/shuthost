//! Static asset serving for the coordinator WebUI.
//!
//! Provides Axum routes to serve HTML, JS, CSS, images, and manifest.

use std::time::Duration;

use axum::{
    Router,
    extract::State,
    response::{IntoResponse, Redirect},
    routing::get,
};
use axum_extra::{
    TypedHeader,
    headers::{CacheControl, ContentLength, ContentType},
};
use mime::{TEXT_CSS, IMAGE_SVG};

use crate::{
    auth::Resolved,
    http::{AppState, EXPECTED_AUTH_EXCEPTIONS_VERSION},
};

#[allow(nonstandard_style, reason = "the functions should be const, in lack of that use a fn")]
fn IMMUTABLE_HEADER() -> TypedHeader<CacheControl> {
    TypedHeader(
        CacheControl::new()
            .with_immutable()
            .with_public()
            .with_max_age(Duration::from_secs(31536000)),
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
        .route("/about", get(serve_about))
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
        .route(
            concat!(
                "/architecture_simplified.",
                env!("ASSET_HASH_ARCHITECTURE_SIMPLIFIED_SVG"),
                ".svg"
            ),
            get(serve_architecture_simplified),
        )
        .route(
            concat!(
                "/architecture.",
                env!("ASSET_HASH_ARCHITECTURE_SVG"),
                ".svg"
            ),
            get(serve_architecture_complete),
        )
}

/// Macro to define a static SVG download handler using include_bytes!
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
pub(crate) enum UiMode<'a> {
    Normal {
        config_path: &'a std::path::Path,
        show_logout: bool,
        maybe_auth_warning: &'a str,
    },
    Demo {
        subpath: &'a str,
    },
}

/// Renders the main HTML template, injecting dynamic content and demo disclaimer if needed.
pub(crate) fn render_ui_html(mode: &UiMode<'_>) -> String {
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
    let maybe_demo_disclaimer = match *mode {
        UiMode::Demo { subpath } => {
            // Build a small disclaimer div with a `data-subpath` attribute so the
            // frontend demo code can adapt links and installer commands.
            include_utf8_asset!("partials/demo_disclaimer.html").replace("{ subpath }", subpath)
        }
        _ => "".to_string(),
    };
    let maybe_auth_warning = match *mode {
        UiMode::Normal {
            maybe_auth_warning, ..
        } => maybe_auth_warning,
        UiMode::Demo { .. } => "",
    };
    let config_path = match *mode {
        UiMode::Normal { config_path, .. } => config_path.to_string_lossy().to_string(),
        UiMode::Demo { .. } => "/this/is/a/demo.toml".to_string(),
    };

    include_utf8_asset!("generated/index.html")
        .replace("{ coordinator_config }", &config_path)
        .replace("{ maybe_auth_warning }", maybe_auth_warning)
        .replace("{ maybe_logout }", maybe_logout)
        .replace("{ maybe_demo_disclaimer }", &maybe_demo_disclaimer)
}

/// Serves the main HTML template, injecting dynamic content.
#[axum::debug_handler]
pub(crate) async fn serve_ui(
    State(AppState {
        config_path, auth, ..
    }): State<AppState>,
) -> impl IntoResponse {
    type A = Resolved;

    let show_logout = !matches!(auth.mode, A::Disabled | A::External { .. });

    // Determine whether to include the external auth config warning. If Auth is
    // Disabled we must show it. If Auth::External is configured but its
    // exceptions_version doesn't match the current expected version, show it.
    let maybe_auth_warning = match &auth.mode {
        &A::Token { .. }
        | &A::Oidc { .. }
        | &A::External {
            exceptions_version: EXPECTED_AUTH_EXCEPTIONS_VERSION,
        } => "",
        &A::Disabled | &A::External { .. } => {
            include_utf8_asset!("partials/external_auth_config.tmpl.html")
        }
    };

    (
        TypedHeader(ContentType::html()),
        render_ui_html(&UiMode::Normal {
            config_path: &config_path,
            show_logout,
            maybe_auth_warning,
        }),
    )
}

/// Serves the about and licenses page HTML.
#[axum::debug_handler]
pub(crate) async fn serve_about() -> impl IntoResponse {
    (
        TypedHeader(ContentType::html()),
        include_utf8_asset!("generated/about.html"),
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
        include_utf8_asset!("generated/styles.css"),
    )
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
