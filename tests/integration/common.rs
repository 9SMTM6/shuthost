//! Common utilities for integration tests.
//!
//! This module provides shared functions and types used across multiple integration test modules,
//! such as spawning processes, managing ports, and waiting for services to be ready.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind to address")
        .local_addr()
        .unwrap()
        .port()
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
