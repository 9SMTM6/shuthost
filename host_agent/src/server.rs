//! Server module: listens for TCP connections to process commands and optionally perform shutdown.

use std::{
    env,
    io::{Read as _, Write as _},
    net::{TcpListener, TcpStream},
};

use clap::Parser;
use miniserde::json;
use secrecy::SecretString;
use shuthost_common::{
    CoordinatorMessage, UnwrapToStringExt as _, create_signed_message,
    protocol::{BroadcastMessage, StartupBroadcast},
};

use crate::{
    commands::execute_shutdown,
    install::default_hostname,
    install::{get_default_interface, get_ip, get_mac},
    validation::validate_request,
};

/// Configuration options for running the `host_agent` service.
#[derive(Debug, Parser, Clone)]
pub struct ServiceOptions {
    /// TCP port to listen on for incoming HMAC-signed commands.
    #[arg(long, short, default_value_t = shuthost_common::DEFAULT_AGENT_TCP_PORT)]
    pub port: u16,

    /// UDP port to send startup broadcasts on (where the coordinator will
    /// listen).  This is configured by the coordinator and embedded in the
    /// install command shown in the web UI, so agents start with the right
    /// value even when it differs from the default.
    #[arg(long, short = 'b', default_value_t = shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT)]
    pub broadcast_port: u16,

    /// Shell command used to perform shutdown when requested.
    #[arg(long, short = 'c', default_value_t = get_default_shutdown_command())]
    pub shutdown_command: String,

    /// Shared secret for validating incoming HMAC-signed requests.
    /// Usually set from environment variables, after parsing.
    #[arg(skip)]
    pub shared_secret: Option<SecretString>,

    /// Hostname of this machine.
    #[arg(long, short = 'n', default_value_t = default_hostname())]
    pub hostname: String,
}

/// Starts the TCP listener and handles incoming client connections in sequence.
///
/// # Panics
///
/// Panics if the `SHUTHOST_SHARED_SECRET` environment variable is not set (and the value wasn't smuggled into `ServiceArgs`).
pub(crate) fn start_host_agent(mut config: ServiceOptions) {
    config.shared_secret.get_or_insert_with(|| {
        SecretString::from(
            env::var("SHUTHOST_SHARED_SECRET")
                .expect("SHUTHOST_SHARED_SECRET environment variable must be set or injected"),
        )
    });
    let port = config.port;
    let addr = format!("0.0.0.0:{port}");
    let listener =
        TcpListener::bind(&addr).unwrap_or_else(|_| panic!("Failed to bind port {addr}"));
    println!("Listening on {addr}");

    // TODO:
    // * the code that listens for this on the coordinator is still not implemented.
    broadcast_startup(&config);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let action = handle_client(stream, &config);
                use CoordinatorMessage as M;
                match action {
                    Some(M::Shutdown) => {
                        print!(
                            "Shutdown requested. Executing shutdown command {}... ",
                            config.shutdown_command
                        );
                        execute_shutdown(&config).expect("failed to execute shutdown command");
                    }
                    Some(M::Abort) => {
                        println!("Abort requested. Stopping host_agent service.");
                        break;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("Connection failed: {e}");
            }
        }
    }
}

fn broadcast_startup(config: &ServiceOptions) {
    let interface = get_default_interface().unwrap_or_else(|| "unknown".to_string());
    let ip_address = get_ip(&interface).unwrap_or_else(|| "unknown".to_string());
    let mac_address = get_mac(&interface).unwrap_or_else(|| "unknown".to_string());
    let agent_version = env!("CARGO_PKG_VERSION").to_string();
    let timestamp = shuthost_common::unix_time_seconds();
    let broadcast = BroadcastMessage::AgentStartup(StartupBroadcast {
        hostname: config.hostname.clone(),
        agent_version,
        port: config.port,
        mac_address,
        ip_address,
        timestamp,
    });
    // today we only send the raw startup structure; the enum exists for future
    // expansion but is not serialized directly because the JSON format hasn't
    // been defined yet.
    let message = json::to_string(&broadcast);
    let signed_message = create_signed_message(
        &message,
        config
            .shared_secret
            .as_ref()
            .expect("Shared secret should be set by now"),
    );
    match shuthost_common::create_broadcast_socket(0) {
        Ok(socket) => {
            let broadcast_addr = format!("255.255.255.255:{}", config.broadcast_port);
            if let Err(e) = socket.send_to(signed_message.as_bytes(), &broadcast_addr) {
                eprintln!("Failed to send startup broadcast: {e}");
            } else {
                println!("Sent startup broadcast to {broadcast_addr}");
            }
        }
        Err(e) => eprintln!("Failed to create broadcast socket: {e}"),
    }
}

/// Handles a client connection: reads data, invokes handler, writes response, and triggers shutdown if needed.
/// Returns the action to take after handling the request.
fn handle_client(mut stream: TcpStream, config: &ServiceOptions) -> Option<CoordinatorMessage> {
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
            use CoordinatorMessage as M;
            let result = validate_request(data, config);
            let (response_bytes, action) = match result {
                Ok(M::Status) => (b"OK: status".to_vec(), None),
                Ok(M::Shutdown) => (
                    format!(
                        "Now executing command: {}. Hopefully goodbye.",
                        config.shutdown_command
                    )
                    .into_bytes(),
                    Some(M::Shutdown),
                ),
                Ok(M::Abort) => (b"OK: aborting service".to_vec(), Some(M::Abort)),
                Err(msg) => {
                    eprintln!("Validation error from {peer_addr}: {msg}");
                    (msg.as_bytes().to_vec(), None)
                }
            };
            if let Err(e) = stream.write_all(&response_bytes) {
                eprintln!("Failed to write response to stream ({peer_addr}): {e}");
            }
            action
        }
        Err(e) => {
            eprintln!("Failed to read from stream ({peer_addr}): {e}");
            None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_options_default_ports() {
        let opts = ServiceOptions::parse_from(["shuthost_host_agent"]);
        assert_eq!(opts.port, shuthost_common::DEFAULT_AGENT_TCP_PORT);
        assert_eq!(
            opts.broadcast_port,
            shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT
        );
        // shutdown_command and hostname have reasonable defaults but we don't assert them here.
    }

    #[test]
    fn service_options_custom_ports() {
        let opts = ServiceOptions::parse_from([
            "shuthost_host_agent",
            "--port",
            "1234",
            "--broadcast-port",
            "4321",
        ]);
        assert_eq!(opts.port, 1234);
        assert_eq!(opts.broadcast_port, 4321);
    }
}
