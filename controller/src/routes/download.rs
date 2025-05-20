use axum::{
    Router,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
};

use crate::http::AppState;

// Macro to define a handler function from a static binary
macro_rules! agent_handler {
    ($name:ident, $agent_target:expr) => {
        async fn $name() -> impl IntoResponse {
            const AGENT_BINARY: &'static [u8] = include_bytes!(concat!(
                "../../../target/",
                $agent_target,
                "/release/shuthost_agent"
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
agent_handler!(agent_macos_aarch64, "aarch64-apple-darwin");
agent_handler!(agent_macos_x86_64, "x86_64-apple-darwin");
agent_handler!(agent_linux_x86_64, "x86_64-unknown-linux-gnu");
agent_handler!(agent_linux_aarch64, "aarch64-unknown-linux-gnu");
agent_handler!(agent_linux_musl_x86_64, "x86_64-unknown-linux-musl");
agent_handler!(agent_linux_musl_aarch64, "aarch64-unknown-linux-musl");

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
        .route("/agent/macos/aarch64", get(agent_macos_aarch64))
        .route("/agent/macos/x86_64", get(agent_macos_x86_64))
        .route("/agent/linux/x86_64", get(agent_linux_x86_64))
        .route("/agent/linux/aarch64", get(agent_linux_aarch64))
        .route("/agent/linux-musl/x86_64", get(agent_linux_musl_x86_64))
        .route("/agent/linux-musl/aarch64", get(agent_linux_musl_aarch64))
}
