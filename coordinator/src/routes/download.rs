use axum::{
    Router,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
};

use crate::http::AppState;

use super::m2m::download_client_script;

// Macro to define a handler function from a static binary
macro_rules! host_agent_handler {
    ($name:ident, $host_agent_target:expr) => {
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
    ($name:ident, $host_agent_target:expr, feature=$feature:expr) => {
        #[cfg(feature = $feature)]
        host_agent_handler!($name, $host_agent_target);
        #[cfg(not(feature = $feature))]
        async fn $name() -> impl IntoResponse {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body("This agent is not available in this build.".into_response())
                .unwrap()
        }
    };
}

// Generate all handlers
host_agent_handler!(
    host_agent_macos_aarch64,
    "aarch64-apple-darwin",
    feature = "include_macos_agents"
);
host_agent_handler!(
    host_agent_macos_x86_64,
    "x86_64-apple-darwin",
    feature = "include_macos_agents"
);
host_agent_handler!(
    host_agent_linux_x86_64,
    "x86_64-unknown-linux-gnu",
    feature = "include_linux_agents"
);
host_agent_handler!(
    host_agent_linux_aarch64,
    "aarch64-unknown-linux-gnu",
    feature = "include_linux_agents"
);
host_agent_handler!(
    host_agent_linux_musl_x86_64,
    "x86_64-unknown-linux-musl",
    feature = "include_linux_agents"
);
host_agent_handler!(
    host_agent_linux_musl_aarch64,
    "aarch64-unknown-linux-musl",
    feature = "include_linux_agents"
);

async fn get_installer() -> impl IntoResponse {
    const INSTALLER: &[u8] = include_bytes!("./host_agent_installer.sh");
    Response::builder()
        .header("Content-Type", "text/plain")
        .header("Content-Length", INSTALLER.len().to_string())
        .status(StatusCode::OK)
        .body(INSTALLER.into_response())
        .unwrap()
}

async fn get_client_installer() -> impl IntoResponse {
    const INSTALLER: &[u8] = include_bytes!("./client_installer.sh");
    Response::builder()
        .header("Content-Type", "text/plain")
        .header("Content-Length", INSTALLER.len().to_string())
        .status(StatusCode::OK)
        .body(INSTALLER.into_response())
        .unwrap()
}

pub fn get_download_router() -> Router<AppState> {
    Router::new()
        .route("/host_agent_installer.sh", get(get_installer))
        .route("/client_installer.sh", get(get_client_installer))
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
