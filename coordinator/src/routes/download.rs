use axum::{
    Router,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
};

use crate::http::AppState;

/// Macro to define a download handler function for a static plain text document
macro_rules! static_text_download_handler {
    (fn $name:ident, file=$file:expr) => {
        async fn $name() -> impl IntoResponse {
            const DOC: &[u8] = include_bytes!($file);
            Response::builder()
                .header("Content-Type", "text/plain")
                .header("Content-Length", DOC.len().to_string())
                .status(StatusCode::OK)
                .body(DOC.into_response())
                .unwrap()
        }
    };
}

// Generate all agent binary handlers
macro_rules! host_agent_handler {
    (fn $name:ident, target=$host_agent_target:expr) => {
        async fn $name() -> impl IntoResponse {
            const AGENT_BINARY: &'static [u8] = include_bytes!(concat!(
                "../../../target/",
                $host_agent_target,
                "/release/shuthost_host_agent"
            ));
            Response::builder()
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", AGENT_BINARY.len().to_string())
                .status(StatusCode::OK)
                .body(AGENT_BINARY.into_response())
                .unwrap()
        }
    };
    (fn $name:ident, target=$host_agent_target:expr, feature=$feature:expr) => {
        #[cfg(feature = $feature)]
        host_agent_handler!(fn $name, target=$host_agent_target);
        #[cfg(not(feature = $feature))]
        async fn $name() -> impl IntoResponse {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body("This agent is not available in this build.".into_response())
                .unwrap()
        }
    };
}

host_agent_handler!(
    fn host_agent_macos_aarch64,
    target = "aarch64-apple-darwin",
    feature = "include_macos_agents"
);
host_agent_handler!(
    fn host_agent_macos_x86_64,
    target = "x86_64-apple-darwin",
    feature = "include_macos_agents"
);
host_agent_handler!(
    fn host_agent_linux_x86_64,
    target = "x86_64-unknown-linux-gnu",
    feature = "include_linux_agents"
);
host_agent_handler!(
    fn host_agent_linux_aarch64,
    target = "aarch64-unknown-linux-gnu",
    feature = "include_linux_agents"
);
host_agent_handler!(
    fn host_agent_linux_musl_x86_64,
    target = "x86_64-unknown-linux-musl",
    feature = "include_linux_agents"
);
host_agent_handler!(
    fn host_agent_linux_musl_aarch64,
    target = "aarch64-unknown-linux-musl",
    feature = "include_linux_agents"
);

static_text_download_handler!(fn download_host_agent_installer, file = "host_agent_installer.sh");
static_text_download_handler!(fn download_client_installer, file = "client_installer.sh");
static_text_download_handler!(fn download_client_script, file = "m2m/shuthost_client.tmpl.sh");

pub fn get_download_router() -> Router<AppState> {
    Router::new()
        .route(
            "/host_agent_installer.sh",
            get(download_host_agent_installer),
        )
        .route("/client_installer.sh", get(download_client_installer))
        .route("/shuthost_client", get(download_client_script))
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
