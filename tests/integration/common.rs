//! Common utilities for integration tests.
//!
//! This module provides shared functions and types used across multiple integration test modules,
//! such as spawning processes, managing ports, and waiting for services to be ready.

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

static NEXT_PORT: AtomicU16 = AtomicU16::new(10000);

pub fn get_free_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::SeqCst)
}

/// Guard that kills and waits on a child process when dropped.
pub struct KillOnDrop(pub Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn get_coordinator_bin() -> &'static str {
    env!("CARGO_BIN_EXE_coordinator")
}

pub fn get_agent_bin() -> &'static str {
    env!("CARGO_BIN_EXE_host_agent")
}

/// Spawn the coordinator service from a given config string.
/// Writes the config to a temp file and spawns the coordinator binary.
pub fn spawn_coordinator_with_config(port: u16, config_toml: &str) -> Child {
    let tmp = std::env::temp_dir().join(format!("integration_test_config_{}.toml", port));
    std::fs::write(&tmp, config_toml).expect("failed to write config");

    spawn_coordinator_with_config_file(&tmp)
}

/// Spawn the coordinator service from a given config file path.
pub fn spawn_coordinator_with_config_file(config_path: &std::path::Path) -> Child {
    let bin = get_coordinator_bin();

    // Prefer built binary when running under `cargo test`.
    Command::new(bin)
        .args(["control-service", "--config", config_path.to_str().unwrap()])
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to start coordinator")
}

/// Spawn the host agent binary with optional env pairs and args.
pub fn spawn_host_agent_with_env_args(envs: &[(&str, &str)], args: &[&str]) -> Child {
    let bin = get_agent_bin();
    let mut cmd = Command::new(bin);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.args(args)
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to start host_agent")
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
