// Integration tests for shuthost_coordinator and shuthost_host_agent
// Place integration tests here for API, config, WOL, and binary startup functionality

mod common;

use reqwest::Client;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command};
// ...existing code...

use common::{KillOnDrop, get_free_port, spawn_coordinator_with_config, wait_for_listening};

fn get_agent_bin() -> String {
    // Ensure the agent binary is built before returning the path
    let status = std::process::Command::new("cargo")
        .args(["build", "--bin", "shuthost_host_agent"])
        .status()
        .expect("failed to run cargo build for shuthost_host_agent");
    assert!(
        status.success(),
        "cargo build for shuthost_host_agent failed"
    );
    std::env::current_dir()
        .unwrap()
        .join("target/debug/shuthost_host_agent")
        .to_string_lossy()
        .into_owned()
}

/// Run the host agent binary and return its Output (useful for `--help` checks).
pub fn run_host_agent_output(args: &[&str]) -> std::process::Output {
    // Prefer built binary when running under `cargo test`.
    let bin = get_agent_bin();
    Command::new(bin)
        .args(args)
        .output()
        .expect("failed to run host_agent")
}

/// Spawn the host agent binary with optional env pairs and args.
pub fn spawn_host_agent_with_env_args(envs: &[(&str, &str)], args: &[&str]) -> Child {
    let bin = get_agent_bin();
    let mut cmd = Command::new(bin);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.args(args).spawn().expect("failed to start host_agent")
}

/// Convenience wrapper when no extra env vars are required.
pub fn spawn_host_agent(args: &[&str]) -> Child {
    spawn_host_agent_with_env_args(&[], args)
}

#[tokio::test]
async fn test_coordinator_config_loads() {
    let port = get_free_port();
    let mut child = spawn_coordinator_with_config(
        port,
        &format!(
            r#"
        [server]
        port = {port}
        bind = "127.0.0.1"

        [hosts]

        [clients]
        "#
        ),
    );
    wait_for_listening(port, 2).await;
    let _ = child.kill();
    let status = child.wait().expect("failed to wait on child");
    #[cfg(unix)]
    assert!(
        status.success() || status.code() == Some(0) || status.signal() == Some(9),
        "Process did not start or exit as expected"
    );
    #[cfg(not(unix))]
    assert!(
        status.success() || status.code() == Some(0),
        "Process did not start or exit as expected"
    );
}

#[test]
fn test_host_agent_binary_runs() {
    // Use helper to run the built binary (respects CARGO_BIN_EXE_ env when present)
    let output = run_host_agent_output(["--help"].as_slice());
    assert!(output.status.success() || output.status.code() == Some(0));
}

#[tokio::test]
async fn test_coordinator_and_agent_online_status() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.testhost]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "testsecret"

        [clients]
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    let agent = spawn_host_agent_with_env_args(
        [("SHUTHOST_SHARED_SECRET", "testsecret")].as_slice(),
        ["service", "--port", &agent_port.to_string()].as_slice(),
    );
    let _agent_guard = KillOnDrop(agent);
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let client = Client::new();
    let url = format!("http://127.0.0.1:{coord_port}/api/hosts_status");
    let resp = client
        .get(&url)
        .send()
        .await
        .expect("failed to query hosts_status");
    assert!(resp.status().is_success());
    let json: serde_json::Value = resp.json().await.expect("invalid json");
    assert_eq!(json["testhost"], true, "Host should be online");
}

#[tokio::test]
async fn test_shutdown_command_execution() {
    let shutdown_file = std::env::temp_dir().join("shuthost_shutdown_test");
    let coord_port = get_free_port();
    let agent_port = get_free_port();

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.testhost]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "testsecret"

        [clients]
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    let agent = spawn_host_agent_with_env_args(
        [("SHUTHOST_SHARED_SECRET", "testsecret")].as_slice(),
        [
            "service",
            "--port",
            &agent_port.to_string(),
            "--shutdown-command",
            &format!(
                "echo SHUTDOWN > {shutdown_file}",
                shutdown_file = shutdown_file.to_string_lossy()
            ),
        ]
        .as_slice(),
    );
    let _agent_guard = KillOnDrop(agent);
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let client = Client::new();
    let status_url = format!("http://127.0.0.1:{coord_port}/api/hosts_status");
    let mut online = false;
    for _ in 0..10 {
        let resp = client.get(&status_url).send().await;
        if let Ok(resp) = resp
            && let Ok(json) = resp.json::<serde_json::Value>().await
            && json["testhost"] == true
        {
            online = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    assert!(online, "Host should be online before triggering shutdown");

    let url = format!("http://127.0.0.1:{coord_port}/api/lease/testhost/release");
    let resp = client
        .post(&url)
        .send()
        .await
        .expect("failed to send shutdown lease");
    assert!(resp.status().is_success());

    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    if shutdown_file.exists() {
        let contents = std::fs::read_to_string(&shutdown_file).unwrap_or_default();
        println!("Shutdown file contents: {contents}");
    }
    assert!(
        shutdown_file.exists(),
        "Shutdown file should exist after shutdown command"
    );
    let _ = std::fs::remove_file(shutdown_file); // Clean up after test
}
