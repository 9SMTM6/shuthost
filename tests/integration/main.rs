//! Uses the single integration test approach.
//!
//! This improves parallelism when running the tests, and reduces the number of binaries that have to be built (and linked)

mod common;
mod host_agent;
mod leases;
mod login_error_redirects;
mod token_login;
mod websocket;

use reqwest::Client;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use common::{
    KillOnDrop, get_free_port, spawn_coordinator_with_config, spawn_host_agent_with_env_args,
    wait_for_agent_ready, wait_for_listening,
};

#[tokio::test]
async fn test_coordinator_config_loads() {
    let port = get_free_port();
    let child = spawn_coordinator_with_config(
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
    let mut drop_guard = KillOnDrop(child);
    wait_for_listening(port, 2).await;
    let _ = drop_guard.0.kill();
    let status = drop_guard.0.wait().expect("failed to wait on child");
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
    let shared_secret = "testsecret";

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
        shared_secret = "{shared_secret}"

        [clients]
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    let agent = spawn_host_agent_with_env_args(
        [("SHUTHOST_SHARED_SECRET", shared_secret)].as_slice(),
        ["service", "--port", &agent_port.to_string()].as_slice(),
    );
    let _agent_guard = KillOnDrop(agent);

    // Wait for agent to be ready
    wait_for_agent_ready(agent_port, shared_secret, 5).await;

    let client = Client::new();
    let url = format!("http://127.0.0.1:{coord_port}/api/hosts_status");
    let mut online = false;
    for _ in 0..10 {
        let resp = client.get(&url).send().await;
        if let Ok(resp) = resp
            && let Ok(json) = resp.json::<serde_json::Value>().await
            && json["testhost"] == true
        {
            online = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    assert!(online, "Host should be online");
}
