//! Server module: listens for TCP connections to process commands and optionally perform shutdown.

use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use clap::Parser;
use secrecy::SecretString;
use shuthost_common::UnwrapToStringExt;

use crate::{
    commands::execute_shutdown,
    validation::{Action, validate_request},
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
    pub shared_secret: Option<SecretString>,
}

/// Starts the TCP listener and handles incoming client connections in sequence.
///
/// # Panics
///
/// Panics if the `SHUTHOST_SHARED_SECRET` environment variable is not set (and the value wasn't smuggled into ServiceArgs).
pub(crate) fn start_host_agent(mut config: ServiceOptions) {
    config.shared_secret.get_or_insert_with(|| {
        SecretString::from(
            env::var("SHUTHOST_SHARED_SECRET")
                .expect("SHUTHOST_SHARED_SECRET environment variable must be set or injected"),
        )
    });
    let addr = format!("0.0.0.0:{}", config.port);
    let listener =
        TcpListener::bind(&addr).unwrap_or_else(|_| panic!("Failed to bind port {addr}"));
    println!("Listening on {addr}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let action = handle_client(stream, &config);
                match action {
                    Action::Shutdown => {
                        execute_shutdown(&config).expect("failed to execute shutdown command");
                    }
                    Action::Abort => {
                        println!("Abort requested. Stopping host_agent service.");
                        break;
                    }
                    Action::None => {}
                }
            }
            Err(e) => {
                eprintln!("Connection failed: {e}");
            }
        }
    }
}

/// Handles a client connection: reads data, invokes handler, writes response, and triggers shutdown if needed.
/// Returns the action to take after handling the request.
fn handle_client(mut stream: TcpStream, config: &ServiceOptions) -> Action {
    let mut buffer = [0u8; 1024];
    let peer_addr = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_to_string("unknown");
    match stream.read(&mut buffer) {
        Ok(size) => {
            let Some(data) = buffer.get(..size) else {
                unreachable!("Read data size should always be valid, as its >= buffer size");
            };
            let (response, action) = validate_request(data, config, &peer_addr);
            if let Err(e) = stream.write_all(response.as_bytes()) {
                eprintln!("Failed to write response to stream ({peer_addr}): {e}");
            }
            action
        }
        Err(e) => {
            eprintln!("Failed to read from stream ({peer_addr}): {e}");
            Action::None
        }
    }
}

/// Returns the default shutdown command for this OS and init system.
pub(crate) fn get_default_shutdown_command() -> String {
    #[cfg(target_os = "linux")]
    return if shuthost_common::is_systemd() {
        "systemctl poweroff"
    } else {
        "poweroff"
    }
    .to_string();
    #[cfg(target_os = "macos")]
    return "shutdown -h now".to_string();
    #[cfg(target_os = "windows")]
    return "shutdown /s /t 0".to_string();
}
