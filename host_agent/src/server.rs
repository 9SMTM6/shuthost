//! Server module: listens for TCP connections to process commands and optionally perform shutdown.

use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use clap::Parser;

use crate::{
    commands::execute_shutdown, install::get_default_shutdown_command, validation::validate_request,
};

/// Configuration options for running the host_agent service.
#[derive(Debug, Parser, Clone)]
pub struct ServiceOptions {
    /// TCP port to listen on for incoming HMAC-signed commands.
    #[arg(long = "port", default_value_t = 9090)]
    pub port: u16,

    /// Shell command used to perform shutdown when requested.
    #[arg(long = "shutdown-command", default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    /// Shared secret for validating incoming HMAC-signed requests.
    /// Usually set from environment variables, after parsing.
    #[clap(skip)]
    pub shared_secret: Option<String>,
}

/// Starts the TCP listener and handles incoming client connections in sequence.
///
/// # Panics
///
/// Panics if the `SHUTHOST_SHARED_SECRET` environment variable is not set (and the value wasn't smuggled into ServiceArgs).
pub fn start_host_agent(mut config: ServiceOptions) {
    config.shared_secret.get_or_insert_with(|| {
        env::var("SHUTHOST_SHARED_SECRET")
            .expect("SHUTHOST_SHARED_SECRET environment variable must be set or injected")
    });
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).expect("Failed to bind port");
    println!("Listening on {addr}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream, &config);
            }
            Err(e) => {
                eprintln!("Connection failed: {e}");
            }
        }
    }
}

/// Handles a client connection: reads data, invokes handler, writes response, and triggers shutdown if needed.
fn handle_client(mut stream: TcpStream, config: &ServiceOptions) {
    let mut buffer = [0u8; 1024];
    let peer_addr = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    match stream.read(&mut buffer) {
        Ok(size) => {
            let Some(data) = buffer.get(..size) else {
                unreachable!("Read data size should always be valid, as its >= buffer size");
            };
            let (response, should_shutdown) = validate_request(data, config, &peer_addr);
            if let Err(e) = stream.write_all(response.as_bytes()) {
                eprintln!("Failed to write response to stream ({peer_addr}): {e}");
            }
            if should_shutdown {
                execute_shutdown(config).unwrap();
            }
        }
        Err(e) => {
            eprintln!("Failed to read from stream ({peer_addr}): {e}");
        }
    }
}
