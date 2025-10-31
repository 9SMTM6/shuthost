//! Common utilities for integration tests.
//!
//! This module provides shared functions and types used across multiple integration test modules,
//! such as spawning processes, managing ports, and waiting for services to be ready.
#![cfg_attr(
    coverage,
    expect(
        unreachable_patterns,
        reason = "For some reason clippy sets coverage cfg?"
    )
)]

use clap::Parser;
use shuthost_coordinator::cli::Cli as CoordinatorCli;
use shuthost_host_agent::Cli as AgentCli;
use std::io::Write;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

static NEXT_PORT: AtomicU16 = AtomicU16::new(10000);

pub fn get_free_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::SeqCst)
}

/// Guard that kills the coordinator or agent when dropped.
pub enum KillOnDrop {
    Coordinator(tokio::task::JoinHandle<()>),
    Agent {
        thread: Option<std::thread::JoinHandle<()>>,
        port: u16,
        secret: String,
    },
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        match self {
            KillOnDrop::Coordinator(handle) => {
                handle.abort();
            }
            KillOnDrop::Agent {
                thread,
                port,
                secret,
            } => {
                // Send abort command to the agent
                if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", *port)) {
                    let signed_message = shuthost_common::create_signed_message("abort", secret);
                    drop(stream.write_all(signed_message.as_bytes()));
                }
                if let Some(handle) = thread.take() {
                    drop(handle.join());
                }
            }
        }
    }
}

pub fn get_agent_bin() -> &'static str {
    env!("CARGO_BIN_EXE_host_agent")
}

/// Spawn the coordinator service from a given config string.
/// Writes the config to a temp file and spawns the coordinator binary.
pub fn spawn_coordinator_with_config(port: u16, config_toml: &str) -> KillOnDrop {
    let tmp = std::env::temp_dir().join(format!("integration_test_config_{}.toml", port));
    std::fs::write(&tmp, config_toml).expect("failed to write config");

    spawn_coordinator_with_config_file(&tmp)
}

/// Spawn the coordinator service from a given config file path.
pub fn spawn_coordinator_with_config_file(config_path: &std::path::Path) -> KillOnDrop {
    let cli = CoordinatorCli::parse_from(&[
        "shuthost_coordinator",
        "control-service",
        "--config",
        config_path.to_str().unwrap(),
    ]);
    let handle = tokio::spawn(async move {
        unsafe {
            std::env::set_var("SHUTHOST_INTEGRATION_TEST", "1");
        }
        shuthost_coordinator::inner_main(cli)
            .await
            .expect("inner_main failed");
    });
    KillOnDrop::Coordinator(handle)
}

/// Spawn the host agent binary with the given secret, port, and shutdown command.
pub fn spawn_host_agent(secret: &str, port: u16, shutdown_command: &str) -> KillOnDrop {
    let args = [
        "shuthost_host_agent",
        "service",
        "--port",
        &port.to_string(),
        "--shutdown-command",
        shutdown_command,
    ];
    let cli = AgentCli::parse_from(&args);
    let mut config = match cli.command {
        shuthost_host_agent::Command::Service(opts) => opts,
        _ => panic!("Expected service command"),
    };
    config.shared_secret = Some(secret.to_string());
    let new_cli = AgentCli {
        command: shuthost_host_agent::Command::Service(config),
    };
    let handle = std::thread::spawn(move || {
        shuthost_host_agent::inner_main(new_cli);
    });
    KillOnDrop::Agent {
        thread: Some(handle),
        port,
        secret: secret.to_string(),
    }
}

/// Spawn a test host agent with the given secret and port.
pub fn spawn_host_agent_default(secret: &str, port: u16) -> KillOnDrop {
    spawn_host_agent(secret, port, "")
}

/// Block until a TCP listener is accepting on `127.0.0.1:port` or timeout.
pub async fn wait_for_listening(port: u16, timeout_secs: u64) {
    let start = Instant::now();
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        if start.elapsed() > Duration::from_secs(timeout_secs) {
            panic!("server did not start within timeout");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Block until the host agent is ready to accept status requests.
/// Sends a proper HMAC-signed status message to verify the agent is responding correctly.
pub async fn wait_for_agent_ready(port: u16, shared_secret: &str, timeout_secs: u64) {
    let start = Instant::now();
    let addr = format!("127.0.0.1:{}", port);

    while start.elapsed() < Duration::from_secs(timeout_secs) {
        match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
            Ok(Ok(mut stream)) => {
                // Send a proper status request like the coordinator does
                let signed_message =
                    shuthost_common::create_signed_message("status", shared_secret);
                if stream.write_all(signed_message.as_bytes()).await.is_err() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                let mut buf = vec![0u8; 256];
                match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                    Ok(Ok(n)) if n > 0 => {
                        let data = &buf[..n];
                        let resp = String::from_utf8_lossy(data);
                        // Accept any non-error response as ready
                        if !resp.contains("ERROR") {
                            return;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("agent did not become ready within timeout");
}
