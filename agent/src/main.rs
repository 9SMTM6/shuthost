mod config;
mod server;
mod handler;

use std::env;
use std::path::PathBuf;

fn main() {
    let config_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: binary agent <config.toml>");
        std::process::exit(1);
    });

    let config = config::load_config(PathBuf::from(config_path))
        .expect("Failed to load config");

    server::start_agent(config);
}
