use crate::http::AppState;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
};

pub async fn serve_ui(State(AppState { config_path, .. }): State<AppState>) -> impl IntoResponse {
    let styles = include_str!("../assets/styles_output.css");
    let javascript = include_str!("../assets/app.js");

    let html = include_str!("../assets/index.tmpl.html")
        .replace("{coordinator_config}", &config_path.to_string_lossy())
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{version}", env!("CARGO_PKG_VERSION"))
        .replace("/* {styles} */", styles)
        .replace("{ js }", javascript);

    Response::builder()
        .header("Content-Type", "text/html")
        .body(html.into_response())
        .unwrap()
}

pub async fn serve_manifest() -> impl IntoResponse {
    let manifest = include_str!("../assets/manifest.json")
        .replace("{description}", env!("CARGO_PKG_DESCRIPTION"));

    Response::builder()
        .header("Content-Type", "application/json")
        .body(manifest.into_response())
        .unwrap()
}

pub async fn serve_favicon() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "image/svg+xml")
        .body(include_bytes!("../assets/favicon.svg").into_response())
        .unwrap()
}
