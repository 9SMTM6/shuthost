mod config;
mod http;
mod wol;

use config::load_controller_config;
use http::start_http_server;

fn main() {
    let config = load_controller_config("example_controller_config.toml")
        .expect("Failed to load controller config");
    start_http_server(config);
}
