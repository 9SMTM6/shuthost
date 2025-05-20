use axum::{
    Router,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
};

use crate::http::AppState;

// Macro to define a handler function from a static binary
macro_rules! node_agent_handler {
    ($name:ident, $node_agent_target:expr) => {
        async fn $name() -> impl IntoResponse {
            const AGENT_BINARY: &'static [u8] = include_bytes!(concat!(
                "../../../target/",
                $node_agent_target,
                "/release/shuthost_node_agent"
            ));
            Response::builder()
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", AGENT_BINARY.len().to_string())
                .status(StatusCode::OK)
                .body(AGENT_BINARY.into_response())
                .unwrap()
        }
    };
}

// Generate all handlers
node_agent_handler!(node_agent_macos_aarch64, "aarch64-apple-darwin");
node_agent_handler!(node_agent_macos_x86_64, "x86_64-apple-darwin");
node_agent_handler!(node_agent_linux_x86_64, "x86_64-unknown-linux-gnu");
node_agent_handler!(node_agent_linux_aarch64, "aarch64-unknown-linux-gnu");
node_agent_handler!(node_agent_linux_musl_x86_64, "x86_64-unknown-linux-musl");
node_agent_handler!(node_agent_linux_musl_aarch64, "aarch64-unknown-linux-musl");

async fn get_installer() -> impl IntoResponse {
    const INSTALLER: &'static [u8] = include_bytes!("./autoinstall.sh");
    Response::builder()
        .header("Content-Type", "text/plain")
        .header("Content-Length", INSTALLER.len().to_string())
        .status(StatusCode::OK)
        .body(INSTALLER.into_response())
        .unwrap()
}

pub fn get_download_router() -> Router<AppState> {
    Router::new()
        .route("/installer.sh", get(get_installer))
        .route("/node_agent/macos/aarch64", get(node_agent_macos_aarch64))
        .route("/node_agent/macos/x86_64", get(node_agent_macos_x86_64))
        .route("/node_agent/linux/x86_64", get(node_agent_linux_x86_64))
        .route("/node_agent/linux/aarch64", get(node_agent_linux_aarch64))
        .route("/node_agent/linux-musl/x86_64", get(node_agent_linux_musl_x86_64))
        .route("/node_agent/linux-musl/aarch64", get(node_agent_linux_musl_aarch64))
}
