//! Uses the single integration test approach.
//! 
//! This improves parallelism when running the tests, and reduces the number of binaries that have to be built (and linked)

mod common;
mod host_agent;
mod login_error_redirects;
mod token_login;
mod websocket;

use reqwest::Client;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use common::{
    KillOnDrop, get_free_port, spawn_coordinator_with_config, spawn_host_agent_with_env_args,
    wait_for_listening,
};

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
