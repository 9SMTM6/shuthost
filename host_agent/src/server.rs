use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::handler::{execute_shutdown, handle_request_without_shutdown};
use crate::install::get_default_shutdown_command;
use clap::Parser;

/// Struct for the service subcommand
#[derive(Debug, Parser, Clone)]
pub struct ServiceArgs {
    #[arg(long = "port", default_value_t = 9090)]
    pub port: u16,

    #[arg(long = "shutdown-command", default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    #[clap(skip)]
    pub shared_secret: String,
}

pub fn start_host_agent(mut config: ServiceArgs) {
    config.shared_secret = env::var("SHUTHOST_SHARED_SECRET")
        .expect("SHUTHOST_SHARED_SECRET environment variable must be set");
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).expect("Failed to bind port");
    println!("Listening on {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream, config.clone());
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, config: ServiceArgs) {
    let mut buffer = [0u8; 1024];
    let peer_addr = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    match stream.read(&mut buffer) {
        Ok(size) => {
            let data = &buffer[..size];
            let (response, should_shutdown) =
                handle_request_without_shutdown(data, &config, &peer_addr);
            let _ = stream.write_all(response.as_bytes());
            if should_shutdown {
                execute_shutdown(&config).unwrap();
            }
        }
        Err(e) => {
            eprintln!("Failed to read from stream ({}): {}", peer_addr, e);
        }
    }
}
