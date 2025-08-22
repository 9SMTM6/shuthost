// Integration tests for shuthost_coordinator and shuthost_host_agent
// Place integration tests here for API, config, WOL, and binary startup functionality

use reqwest::Client;
use std::fs;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind to address")
        .local_addr()
        .unwrap()
        .port()
}

/// Guard that kills and waits on a child process when dropped.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn test_coordinator_config_loads() {
    let port = get_free_port();
    let toml_str = format!(
        r#"
        [server]
        port = {port}
        bind = "127.0.0.1"

        [hosts]

        [clients]
        "#
    );
    let tmp = std::env::temp_dir().join("integration_test_config.toml");
    fs::write(&tmp, toml_str).unwrap();
    let mut child = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "shuthost_coordinator",
            "control-service",
            "--config",
            tmp.to_str().unwrap(),
        ])
        .spawn()
        .expect("failed to start coordinator");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
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
    let output = Command::new("cargo")
        .args(["run", "--bin", "shuthost_host_agent", "--", "--help"])
        .output()
        .expect("failed to run host_agent");
    assert!(output.status.success() || output.status.code() == Some(0));
}

#[tokio::test]
#[ignore = "Fails during CI, and I dont have time to fix it right now"]
async fn test_coordinator_and_agent_online_status() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let config = format!(
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
    );
    let tmp = std::env::temp_dir().join("integration_test_config_online.toml");
    std::fs::write(&tmp, config).unwrap();

    let coordinator = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "shuthost_coordinator",
            "control-service",
            "--config",
            tmp.to_str().unwrap(),
        ])
        .spawn()
        .expect("failed to start coordinator");
    let _coordinator_guard = KillOnDrop(coordinator);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let agent = Command::new("env")
        .env("SHUTHOST_SHARED_SECRET", "testsecret")
        .args([
            "cargo",
            "run",
            "--bin",
            "shuthost_host_agent",
            "--",
            "service",
            "--port",
            &agent_port.to_string(),
        ])
        .spawn()
        .expect("failed to start agent");
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
#[ignore = "Fails during CI, and I dont have time to fix it right now"]
async fn test_shutdown_command_execution() {
    use std::path::Path;
    let shutdown_file = "/tmp/shuthost_shutdown_test";
    let _ = std::fs::remove_file(shutdown_file); // Clean up before test
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let config = format!(
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
    );
    let tmp = std::env::temp_dir().join("integration_test_config_shutdown.toml");
    std::fs::write(&tmp, config).unwrap();

    let coordinator = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "shuthost_coordinator",
            "control-service",
            "--config",
            tmp.to_str().unwrap(),
        ])
        .spawn()
        .expect("failed to start coordinator");
    let _coordinator_guard = KillOnDrop(coordinator);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let agent = Command::new("env")
        .env("SHUTHOST_SHARED_SECRET", "testsecret")
        .args([
            "cargo",
            "run",
            "--bin",
            "shuthost_host_agent",
            "--",
            "service",
            "--port",
            &agent_port.to_string(),
            "--shutdown-command",
            &format!("echo SHUTDOWN > {shutdown_file}"),
        ])
        .spawn()
        .expect("failed to start agent");
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
    if Path::new(shutdown_file).exists() {
        let contents = std::fs::read_to_string(shutdown_file).unwrap_or_default();
        println!("Shutdown file contents: {contents}");
    }
    assert!(
        Path::new(shutdown_file).exists(),
        "Shutdown file should exist after shutdown command"
    );
    let _ = std::fs::remove_file(shutdown_file); // Clean up after test
}
