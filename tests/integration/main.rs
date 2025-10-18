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

use common::{
    get_free_port, spawn_coordinator_with_config, spawn_host_agent_default, wait_for_agent_ready,
    wait_for_listening,
};

#[tokio::test]
async fn test_coordinator_config_loads() {
    let port = get_free_port();
    let _child = spawn_coordinator_with_config(
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
}

#[tokio::test]
async fn test_coordinator_and_agent_online_status() {
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

    let _agent = spawn_host_agent_default(shared_secret, agent_port);

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

#[tokio::test]
async fn test_lease_persistence_across_restarts() {
    let coord_port = get_free_port();
    let db_path = std::env::temp_dir().join(format!("shuthost_test_{}.db", coord_port));

    // Ensure clean start
    let _ = std::fs::remove_file(&db_path);

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"
        db_path = "{}"

        [hosts]

        [clients]
    "#,
        db_path.to_string_lossy()
    );

    // Start coordinator with database
    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &config,
    );
    wait_for_listening(coord_port, 5).await;

    let client = Client::new();

    // Take a lease via API
    let lease_url = format!("http://127.0.0.1:{coord_port}/api/lease/testhost/take");
    let resp = client
        .post(&lease_url)
        .send()
        .await
        .expect("failed to take lease");
    assert!(resp.status().is_success());

    // Kill coordinator
    drop(coordinator_child);
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Start coordinator again with same db
    let _coordinator_child2 = spawn_coordinator_with_config(
        coord_port,
        &config,
    );
    wait_for_listening(coord_port, 5).await;

    // Verify lease still exists after restart by trying to release it
    let release_url = format!("http://127.0.0.1:{coord_port}/api/lease/testhost/release");
    let resp = client
        .post(&release_url)
        .send()
        .await
        .expect("failed to release lease");
    assert!(resp.status().is_success(), "Lease should exist and be releasable after restart");

    // Clean up
    let _ = std::fs::remove_file(&db_path);
}
