use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::handler::handle_request;
use crate::install::generate_secret;
use clap::Parser;

/// Struct for the service subcommand
#[derive(Debug, Parser, Clone)]
pub struct ServiceArgs {
    #[arg(long = "port", default_value_t = 9090)]
    pub port: u16,

    #[arg(long = "shutdown-command", default_value = "systemctl poweroff")]
    pub shutdown_command: String,

    #[arg(long = "shared-secret", default_value_t = generate_secret())]
    pub shared_secret: String,
}

pub fn start_agent(config: ServiceArgs) {
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).expect("Failed to bind port");
    println!("[agent] Listening on {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream, config.clone());
            }
            Err(e) => {
                eprintln!("[agent] Connection failed: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, config: ServiceArgs) {
    let mut buffer = [0u8; 1024];
    match stream.read(&mut buffer) {
        Ok(size) => {
            let data = &buffer[..size];
            let response = handle_request(data, &config);
            let _ = stream.write_all(response.as_bytes());
        }
        Err(e) => {
            eprintln!("[agent] Failed to read from stream: {}", e);
        }
    }
}
