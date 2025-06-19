// Integration tests for shuthost_coordinator and shuthost_host_agent
// Place integration tests here for API, config, WOL, and binary startup functionality

use std::process::Command;
use std::fs;
use reqwest::Client;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

#[tokio::test]
async fn test_coordinator_config_loads() {
    let toml_str = r#"
        [server]
        port = 12345
        bind = "127.0.0.1"

        [hosts]

        [clients]
        "#;
    let tmp = std::env::temp_dir().join("integration_test_config.toml");
    fs::write(&tmp, toml_str).unwrap();
    let mut child = Command::new("cargo")
        .args(["run", "--bin", "shuthost_coordinator", "control-service", "--config", tmp.to_str().unwrap()])
        .spawn()
        .expect("failed to start coordinator");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = child.kill();
    let status = child.wait().expect("failed to wait on child");
    #[cfg(unix)]
    assert!(status.success() || status.code() == Some(0) || status.signal() == Some(9), "Process did not start or exit as expected");
    #[cfg(not(unix))]
    assert!(status.success() || status.code() == Some(0), "Process did not start or exit as expected");
}

#[test]
fn test_host_agent_binary_runs() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "shuthost_host_agent", "--", "--help"])
        .output()
        .expect("failed to run host_agent");
    assert!(output.status.success() || output.status.code() == Some(0));
}

#[tokio::test]
async fn test_coordinator_and_agent_online_status() {
    // Write a config with a host and shared secret
    let config = r#"
        [server]
        port = 60100
        bind = "127.0.0.1"

        [hosts.testhost]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = 60101
        shared_secret = "testsecret"

        [clients]
    "#;
    let tmp = std::env::temp_dir().join("integration_test_config_online.toml");
    std::fs::write(&tmp, config).unwrap();

    // Start the coordinator
    let mut coordinator = Command::new("cargo")
        .args(["run", "--bin", "shuthost_coordinator", "control-service", "--config", tmp.to_str().unwrap()])
        .spawn()
        .expect("failed to start coordinator");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Start the agent with the same shared secret and port
    let mut agent = Command::new("env")
        .env("SHUTHOST_SHARED_SECRET", "testsecret")
        .args(["cargo", "run", "--bin", "shuthost_host_agent", "--", "service", "--port", "60101"])
        .spawn()
        .expect("failed to start agent");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Query the coordinator for host status
    let client = Client::new();
    let url = "http://127.0.0.1:60100/api/hosts_status";
    let resp = client.get(url).send().await.expect("failed to query hosts_status");
    assert!(resp.status().is_success());
    let json: serde_json::Value = resp.json().await.expect("invalid json");
    assert_eq!(json["testhost"], true, "Host should be online");

    let _ = agent.kill();
    let _ = agent.wait();
    let _ = coordinator.kill();
    let _ = coordinator.wait();
}
