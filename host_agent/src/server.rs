//! Server module: listens for TCP connections to process commands and optionally perform shutdown.

use std::{
    env,
    io::{Read as _, Write as _},
    net::{TcpListener, TcpStream},
    process,
};

use clap::Parser;
use miniserde::json;
use secrecy::SecretString;
use shuthost_common::{
    CoordinatorMessage, UnwrapToStringExt as _, create_signed_message,
    protocol::{BroadcastMessage, OsType, StartupBroadcast},
};

use crate::{
    VERSION,
    commands::execute_shutdown,
    install::{
        InitSystem, default_hostname, get_default_interface, get_inferred_init_system, get_ip,
        get_mac,
    },
    registration,
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

    /// Init system or install type for this agent.
    #[arg(long, default_value_t = get_inferred_init_system())]
    pub init_system: InitSystem,

    /// Path to the self-extracting script, only used and allowed for self-extracting installs. Must be absolute.
    #[arg(long)]
    pub script_path: Option<String>,
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
    registration::validate_script_path_args(&registration::Args {
        init_system: config.init_system,
        script_path: config.script_path.clone(),
    })
    .unwrap_or_else(|err| {
        eprintln!("Error: {err}");
        process::exit(1);
    });

    let port = config.port;
    let addr = format!("0.0.0.0:{port}");
    let listener =
        TcpListener::bind(&addr).unwrap_or_else(|_| panic!("Failed to bind port {addr}"));
    println!("Listening on {addr}");

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

fn get_os() -> OsType {
    if cfg!(target_os = "linux") {
        OsType::Linux
    } else if cfg!(target_os = "macos") {
        OsType::MacOS
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        unreachable!("Unsupported OS");
    }
}

fn broadcast_startup(config: &ServiceOptions) {
    let interface = get_default_interface().unwrap_or_else(|| "unknown".to_string());
    let ip_address = get_ip(&interface).unwrap_or_else(|| "unknown".to_string());
    let mac_address = get_mac(&interface).unwrap_or_else(|| "unknown".to_string());
    let agent_version = VERSION.to_string();
    let timestamp = shuthost_common::unix_time_seconds();
    let broadcast = BroadcastMessage::AgentStartup(StartupBroadcast {
        hostname: config.hostname.clone(),
        agent_version,
        port: config.port,
        mac_address,
        ip_address,
        timestamp,
        init_system: config.init_system.into(),
        os: get_os(),
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
                Ok(M::Status) => {
                    let mut fields = vec![
                        format!("agent_version={}", VERSION),
                        format!("init_system={}", config.init_system),
                        format!("os={}", get_os()),
                    ];
                    if let &Some(ref script_path) = &config.script_path {
                        fields.push(format!("script_path={script_path}"));
                    }
                    (
                        format!("OK: status;{}", fields.join("; ")).into_bytes(),
                        None,
                    )
                }
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
    use std::io::{Read as _, Write as _};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use secrecy::SecretString;
    use shuthost_common::create_signed_message;

    use super::*;

    fn make_args(secret: SecretString) -> ServiceOptions {
        ServiceOptions {
            port: 0,
            broadcast_port: 0,
            shutdown_command: "shutdown_cmd".to_string(),
            shared_secret: Some(secret),
            hostname: "test_hostname".to_string(),
            init_system: InitSystem::SelfExtractingShell,
            script_path: None,
        }
    }

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

    #[test]
    fn status_response_includes_extended_info() {
        let secret = SecretString::from("secret");
        let config = make_args(secret.clone());
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server_config = config.clone();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept connection");
            let action = handle_client(stream, &server_config);
            assert_eq!(action, None);
        });

        let mut stream = TcpStream::connect(addr).expect("connect to agent");
        let signed = create_signed_message("status", config.shared_secret.as_ref().unwrap());
        stream
            .write_all(signed.as_bytes())
            .expect("send status request");

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read status response");

        assert!(response.starts_with("OK: status;"));
        assert!(response.contains("agent_version="));
        assert!(response.contains("init_system="));
        assert!(response.contains("os="));

        handle.join().expect("server thread finished");
    }

    #[test]
    fn validate_script_path_args_rejects_relative_script_path() {
        let mut config = make_args(SecretString::from("secret"));
        config.script_path = Some("relative/path/to/script".to_string());
        let args = registration::Args {
            init_system: config.init_system,
            script_path: config.script_path.clone(),
        };

        assert_eq!(
            registration::validate_script_path_args(&args),
            Err("--script-path must be an absolute path".to_string())
        );
    }

    #[test]
    fn validate_script_path_args_rejects_script_path_with_non_self_extracting_init_system() {
        let mut config = make_args(SecretString::from("secret"));
        config.init_system = InitSystem::Systemd;
        config.script_path = Some("/absolute/path/to/script".to_string());
        let args = registration::Args {
            init_system: config.init_system,
            script_path: config.script_path.clone(),
        };

        assert_eq!(
            registration::validate_script_path_args(&args),
            Err("--script-path may only be used with self-extracting init systems".to_string())
        );
    }
}
