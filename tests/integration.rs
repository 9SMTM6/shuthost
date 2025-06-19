// Integration tests for shuthost_coordinator and shuthost_host_agent
// Place integration tests here for API, config, WOL, and binary startup functionality

use std::process::Command;
use std::fs;
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
