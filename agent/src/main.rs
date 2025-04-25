mod server;
mod handler;
mod install;

use std::env;
use std::path::PathBuf;
use std::path::Path;
use install::generate_secret;
use install::install_agent;

use clap::{Parser};
use serde::Deserialize;

#[derive(Deserialize)]
#[derive(Debug, Clone)]
#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = env!("CARGO_PKG_DESCRIPTION"))]
struct Args {
    #[arg(long = "port", default_value_t = 9090)]
    port: u16,

    #[arg(long = "shutdown-command", default_value = "systemctl poweroff")]
    shutdown_command: String,

    #[arg(long = "shared-secret", default_value_t = generate_secret())]
    shared_secret: String,
}


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

    let args = Args::parse();

    server::start_agent(args);
}
