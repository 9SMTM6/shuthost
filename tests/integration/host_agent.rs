//! Integration tests for host_agent functionality

use crate::common::{
    get_agent_bin, get_free_port, spawn_coordinator_with_config, spawn_host_agent,
    wait_for_agent_ready, wait_for_listening,
};

#[test]
fn test_host_agent_binary_runs() {
    // Use helper to run the built binary (respects CARGO_BIN_EXE_ env when present)
    let bin = get_agent_bin();
    let output = std::process::Command::new(bin)
        .args(["--help"])
        .stdout(std::process::Stdio::null())
        .output()
        .expect("failed to run host_agent");
    assert!(output.status.success() || output.status.code() == Some(0));
}

#[tokio::test]
async fn test_shutdown_command_execution() {
    let shutdown_file = std::env::temp_dir().join("shuthost_shutdown_test");
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let shared_secret = "testsecret";

    let _coordinator_child = spawn_coordinator_with_config(
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
        shared_secret = "{shared_secret}"

        [clients]
    "#
        ),
    );
    wait_for_listening(coord_port, 5).await;

    let _agent = spawn_host_agent(
        shared_secret,
        agent_port,
        &format!("echo SHUTDOWN > {}", shutdown_file.to_string_lossy()),
    );

    // Wait for agent to be ready
    wait_for_agent_ready(agent_port, shared_secret, 5).await;

    let client = reqwest::Client::new();
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

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        shutdown_file.exists(),
        "Shutdown file should exist after shutdown command"
    );
    let contents = std::fs::read_to_string(&shutdown_file).unwrap_or_default();
    assert_eq!(
        contents.trim(),
        "SHUTDOWN",
        "Shutdown file should contain 'SHUTDOWN'"
    );
    drop(std::fs::remove_file(shutdown_file)); // Clean up after test
}
