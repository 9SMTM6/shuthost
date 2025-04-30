mod config;
mod http;
mod wol;

use std::{env, fs};

use http::start_http_server;

#[tokio::main]
async fn main() {
    // Get config path from env or fallback
    let config_path_raw = env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "controller_config.toml".to_string());
    let config_path = fs::canonicalize(&config_path_raw)
        .unwrap_or_else(|_| panic!("Config file not found at: {}", config_path_raw));
    println!("Using config path: {}", config_path.display());
    start_http_server(&config_path).await;
}
