use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use axum_extra::{
    TypedHeader,
    headers::{ContentLength, ContentType},
};

use crate::http::AppState;

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
        #[axum::debug_handler]
        async fn $name() -> impl IntoResponse {
            const AGENT_BINARY: &'static [u8] = include_bytes!(concat!(
                "../../../target/",
                $host_agent_target,
                "/release/shuthost_host_agent"
            ));
            (
                TypedHeader(ContentType::from(mime::APPLICATION_OCTET_STREAM)),
                TypedHeader(ContentLength(AGENT_BINARY.len() as u64)),
                AGENT_BINARY,
            )
        }
    };
    (fn $name:ident, target=$host_agent_target:expr, feature=$feature:expr) => {
        #[cfg(feature = $feature)]
        host_agent_handler!(fn $name, target=$host_agent_target);
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
    fn host_agent_linux_x86_64,
    target = "x86_64-unknown-linux-gnu",
    feature = "include_linux_x86_64_agent"
);
host_agent_handler!(
    fn host_agent_linux_aarch64,
    target = "aarch64-unknown-linux-gnu",
    feature = "include_linux_aarch64_agent"
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

static_text_download_handler!(fn download_host_agent_installer, file = "scripts/host_agent_installer.sh");
static_text_download_handler!(fn download_client_installer, file = "scripts/client_installer.sh");
static_text_download_handler!(fn download_client_installer_ps1, file = "scripts/client_installer.ps1");
static_text_download_handler!(fn download_client_script, file = "scripts/shuthost_client.tmpl.sh");
static_text_download_handler!(fn download_client_script_ps1, file = "scripts/shuthost_client.tmpl.ps1");

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/host_agent_installer.sh",
            get(download_host_agent_installer),
        )
        .route("/client_installer.sh", get(download_client_installer))
        .route("/client_installer.ps1", get(download_client_installer_ps1))
        .route("/shuthost_client.sh", get(download_client_script))
        .route("/shuthost_client.ps1", get(download_client_script_ps1))
        .route("/host_agent/macos/aarch64", get(host_agent_macos_aarch64))
        .route("/host_agent/macos/x86_64", get(host_agent_macos_x86_64))
        .route("/host_agent/linux/x86_64", get(host_agent_linux_x86_64))
        .route("/host_agent/linux/aarch64", get(host_agent_linux_aarch64))
        .route(
            "/host_agent/linux-musl/x86_64",
            get(host_agent_linux_musl_x86_64),
        )
        .route(
            "/host_agent/linux-musl/aarch64",
            get(host_agent_linux_musl_aarch64),
        )
}
