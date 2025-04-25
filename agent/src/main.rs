mod config;
mod server;
mod handler;
mod install;

use std::env;
use std::path::PathBuf;
use std::path::Path;
use install::install_agent;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.contains(&"--install".to_string()) {
        let port = args.iter().find(|&x| x.starts_with("--port=")).map(|x| x.split("=").nth(1).unwrap().parse().unwrap());
        let shutdown_command = args.iter().find(|&x| x.starts_with("--shutdown-command=")).map(|x| x.split("=").nth(1).unwrap());
        let secret = args.iter().find(|&x| x.starts_with("--shared-secret=")).map(|x| x.split("=").nth(1).unwrap().to_string());

        match install_agent(Path::new(&args[0]), port, shutdown_command, secret) {
            Ok(_) => println!("Agent installed successfully!"),
            Err(e) => eprintln!("Error installing agent: {}", e),
        }
        return;
    }

    let config_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: binary agent <config.toml>");
        std::process::exit(1);
    });

    let config = config::load_config(PathBuf::from(config_path))
        .expect("Failed to load config");

    server::start_agent(config);
}
