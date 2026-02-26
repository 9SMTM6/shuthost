use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use axum_extra::{
    TypedHeader,
    headers::{ContentLength, ContentType},
};

use crate::app::AppState;

/// Macro to define a download handler function for a static plain text document
macro_rules! static_text_download_handler {
    (fn $name:ident, file=$file:expr) => {
        #[axum::debug_handler]
        async fn $name() -> impl IntoResponse {
            const DOC: &[u8] = include_bytes!(concat!("../../../", $file));
            (
                TypedHeader(ContentType::text()),
                TypedHeader(ContentLength(DOC.len() as u64)),
                DOC,
            )
        }
    };
}

// Generate all agent binary handlers
macro_rules! host_agent_handler {
    (fn $name:ident, target=$host_agent_target:expr) => {
        host_agent_handler!(fn $name, target=$host_agent_target, ext="");
    };
    (fn $name:ident, target=$host_agent_target:expr, ext=$ext:expr) => {
        #[axum::debug_handler]
        async fn $name() -> impl IntoResponse {
            const AGENT_BINARY: &'static [u8] = include_bytes!(concat!(
                "../../../target/",
                $host_agent_target,
                "/release/shuthost_host_agent",
                $ext
            ));
            (
                TypedHeader(ContentType::from(mime::APPLICATION_OCTET_STREAM)),
                TypedHeader(ContentLength(AGENT_BINARY.len() as u64)),
                AGENT_BINARY,
            )
        }
    };
    (fn $name:ident, target=$host_agent_target:expr, feature=$feature:expr) => {
        host_agent_handler!(fn $name, target=$host_agent_target, feature=$feature, ext="");
    };
    (fn $name:ident, target=$host_agent_target:expr, feature=$feature:expr, ext=$ext:expr) => {
        #[cfg(feature = $feature)]
        host_agent_handler!(fn $name, target=$host_agent_target, ext=$ext);
        #[cfg(not(feature = $feature))]
        #[axum::debug_handler]
        async fn $name() -> impl IntoResponse {
            (
                StatusCode::NOT_FOUND,
                "This agent is not available in this build.",
            )
        }
    };
}

host_agent_handler!(
    fn host_agent_macos_aarch64,
    target = "aarch64-apple-darwin",
    feature = "include_macos_aarch64_agent"
);
host_agent_handler!(
    fn host_agent_macos_x86_64,
    target = "x86_64-apple-darwin",
    feature = "include_macos_x86_64_agent"
);
host_agent_handler!(
    fn host_agent_linux_musl_x86_64,
    target = "x86_64-unknown-linux-musl",
    feature = "include_linux_musl_x86_64_agent"
);
host_agent_handler!(
    fn host_agent_linux_musl_aarch64,
    target = "aarch64-unknown-linux-musl",
    feature = "include_linux_musl_aarch64_agent"
);
host_agent_handler!(
    fn host_agent_windows_x86_64,
    target = "x86_64-pc-windows-msvc",
    feature = "include_windows_x86_64_agent",
    ext = ".exe"
);
host_agent_handler!(
    fn host_agent_windows_aarch64,
    target = "aarch64-pc-windows-msvc",
    feature = "include_windows_aarch64_agent",
    ext = ".exe"
);

static_text_download_handler!(fn download_host_agent_installer, file = "scripts/coordinator_installers/host_agent.sh");
static_text_download_handler!(fn download_host_agent_installer_ps1, file = "scripts/coordinator_installers/host_agent.ps1");
static_text_download_handler!(fn download_client_installer, file = "scripts/coordinator_installers/client.sh");
static_text_download_handler!(fn download_client_installer_ps1, file = "scripts/coordinator_installers/client.ps1");
static_text_download_handler!(fn download_client_script, file = "scripts/enduser_templates/shuthost_client.tmpl.sh");
static_text_download_handler!(fn download_client_script_ps1, file = "scripts/enduser_templates/shuthost_client.tmpl.ps1");

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/host_agent_installer.sh",
            get(download_host_agent_installer),
        )
        .route(
            "/host_agent_installer.ps1",
            get(download_host_agent_installer_ps1),
        )
        .route("/client_installer.sh", get(download_client_installer))
        .route("/client_installer.ps1", get(download_client_installer_ps1))
        .route("/shuthost_client.sh", get(download_client_script))
        .route("/shuthost_client.ps1", get(download_client_script_ps1))
        .route("/host_agent/macos/aarch64", get(host_agent_macos_aarch64))
        .route("/host_agent/macos/x86_64", get(host_agent_macos_x86_64))
        .route(
            "/host_agent/linux-musl/x86_64",
            get(host_agent_linux_musl_x86_64),
        )
        .route(
            "/host_agent/linux-musl/aarch64",
            get(host_agent_linux_musl_aarch64),
        )
        .route("/host_agent/windows/x86_64", get(host_agent_windows_x86_64))
        .route(
            "/host_agent/windows/aarch64",
            get(host_agent_windows_aarch64),
        )
}
