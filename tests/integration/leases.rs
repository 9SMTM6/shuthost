//! Integration tests for lease endpoints (API and M2M)

use std::time::Duration;

use reqwest::Client;
use shuthost_common::create_signed_message;

use crate::common::{
    KillOnDrop, get_free_port, spawn_coordinator_with_config, spawn_host_agent_with_env_args,
    wait_for_agent_ready, wait_for_listening,
};

#[tokio::test]
async fn test_m2m_lease_take_and_release() {
    let coord_port = get_free_port();

    let client_id = "test-client-123";
    let client_secret = "clientsecret";

    let agent_port = get_free_port();
    let agent_id = "testhost";
    let agent_secret = "testsecret";

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts."{agent_id}"]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{agent_secret}"

        [clients."{client_id}"]
        shared_secret = "{client_secret}"
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    // Take a lease via M2M endpoint
    let take_url = format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/take",);

    let signed_message = create_signed_message("take", client_secret);

    // start the synchronous (by default) lease request
    let take_lease_req = tokio::spawn(async move {
        let resp = Client::new()
            .post(&take_url)
            .header("X-Client-ID", client_id)
            .header("X-Request", signed_message)
            .send()
            .await
            .expect("failed to take lease");

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| String::from("(no body)"));
            panic!("Lease take failed with status {}: {}", status, body);
        }
    });

    // simulate the WOL request by starting the agent
    let agent_guard = {
        let agent = spawn_host_agent_with_env_args(
            [("SHUTHOST_SHARED_SECRET", agent_secret)].as_slice(),
            [
                "service",
                "--port",
                &agent_port.to_string(),
                "--shutdown-command",
                "",
            ]
            .as_slice(),
        );
        let agent_guard = KillOnDrop(agent);

        // Wait for agent to be listening
        wait_for_agent_ready(agent_port, agent_secret, 5).await;
        agent_guard
    };

    // ensure lease request finished successfully
    take_lease_req.await.unwrap();

    // Release the lease
    let release_url = format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/release",);

    let signed_message = create_signed_message("release", client_secret);

    // start the synchronous (by default) lease request
    let release_lease_req = tokio::spawn(async move {
        let resp = Client::new()
            .post(&release_url)
            .header("X-Client-ID", client_id)
            .header("X-Request", signed_message)
            .send()
            .await
            .expect("failed to release lease");

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| String::from("(no body)"));
            panic!("Lease release failed with status {}: {}", status, body);
        }
    });

    // without sleeping here it seems to fail
    // maybe the scheduler doesn't finish the request before the agent was killed.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Simulate shutdown by killing the agent
    drop(agent_guard);

    // ensure lease request finished successfully
    release_lease_req.await.unwrap();
}

#[tokio::test]
async fn test_m2m_lease_async_take_and_release() {
    let coord_port = get_free_port();

    let client_id = "test-client-123";
    let client_secret = "clientsecret";

    let agent_port = get_free_port();
    let agent_id = "testhost";
    let agent_secret = "testsecret";

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts."{agent_id}"]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{agent_secret}"

        [clients."{client_id}"]
        shared_secret = "{client_secret}"
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    // Take a lease via M2M endpoint
    let take_url =
        format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/take?async=true",);

    let signed_message = create_signed_message("take", client_secret);

    // start the asynchronous lease request. We will not start the agent, but it should still return
    let resp = Client::new()
        .post(&take_url)
        .header("X-Client-ID", client_id)
        .header("X-Request", signed_message)
        .send()
        .await
        .expect("failed to take lease");

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        panic!("Lease take failed with status {}: {}", status, body);
    }

    // Bring host online by starting the agent, to blockade the release request
    let _agent_guard = {
        let agent = spawn_host_agent_with_env_args(
            [("SHUTHOST_SHARED_SECRET", agent_secret)].as_slice(),
            [
                "service",
                "--port",
                &agent_port.to_string(),
                "--shutdown-command",
                "",
            ]
            .as_slice(),
        );
        let agent_guard = KillOnDrop(agent);

        // Wait for agent to be listening
        wait_for_agent_ready(agent_port, agent_secret, 5).await;
        agent_guard
    };

    // Release the lease
    let release_url =
        format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/release?async=true",);

    let signed_message = create_signed_message("release", client_secret);

    // start the synchronous (by default) lease request
    let resp = Client::new()
        .post(&release_url)
        .header("X-Client-ID", client_id)
        .header("X-Request", signed_message)
        .send()
        .await
        .expect("failed to release lease");

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        panic!("Lease release failed with status {}: {}", status, body);
    }
}

#[tokio::test]
async fn test_api_reset_client_leases() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let client_id = "test-client-reset";
    let client_secret = "clientsecret";
    let agent_secret = "testsecret";
    let agent_id = "testhost";

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts."{agent_id}"]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{agent_secret}"

        [clients."{client_id}"]
        shared_secret = "{client_secret}"
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    let client = Client::new();

    // Take a lease via M2M
    let take_url =
        format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/take?async=true",);

    let signed_message = create_signed_message("take", client_secret);

    let resp = client
        .post(&take_url)
        .header("X-Client-ID", client_id)
        .header("X-Request", signed_message)
        .send()
        .await
        .expect("failed to take lease");
    assert!(resp.status().is_success());

    // simulate the online host
    let agent_guard = {
        let agent = spawn_host_agent_with_env_args(
            [("SHUTHOST_SHARED_SECRET", agent_secret)].as_slice(),
            [
                "service",
                "--port",
                &agent_port.to_string(),
                "--shutdown-command",
                "",
            ]
            .as_slice(),
        );
        let agent_guard = KillOnDrop(agent);

        // Wait for agent to be listening
        wait_for_agent_ready(agent_port, agent_secret, 5).await;
        agent_guard
    };

    // Reset all leases for this client - spawn background task
    let reset_url = format!("http://127.0.0.1:{coord_port}/api/reset_leases/{client_id}");
    let reset_task = tokio::spawn(async move {
        let resp = client
            .post(&reset_url)
            .send()
            .await
            .expect("failed to release leases");

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| String::from("(no body)"));
            panic!(
                "Releasing all client leases failed with status {}: {}",
                status, body
            );
        }
    });

    // Simulate shutdown by killing the agent
    drop(agent_guard);

    // Give the reset request time to finish
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Wait for reset to complete
    reset_task.await.unwrap()
}

#[tokio::test]
async fn test_m2m_lease_sync_take_timeout_when_host_offline() {
    let coord_port = get_free_port();

    let client_id = "test-client-sync-timeout";
    let client_secret = "clientsecret";

    let agent_port = get_free_port();
    let agent_id = "testhost";
    let agent_secret = "testsecret";

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts."{agent_id}"]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{agent_secret}"

        [clients."{client_id}"]
        shared_secret = "{client_secret}"
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    // Take a lease synchronously (host remains offline)
    let take_url = format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/take",);

    let signed_message = create_signed_message("take", client_secret);

    // Start the lease request
    let resp = Client::new()
        .post(&take_url)
        .header("X-Client-ID", client_id)
        .header("X-Request", signed_message)
        .send()
        .await
        .expect("Failed to get resp");

    if resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        panic!(
            "Taking client lease succeeded unexpectedly with status {}: {}",
            status, body
        );
    }
}

#[tokio::test]
async fn test_m2m_lease_sync_release_timeout_when_host_online() {
    let coord_port = get_free_port();

    let client_id = "test-client-sync-release-timeout";
    let client_secret = "clientsecret";

    let agent_port = get_free_port();
    let agent_id = "testhost";
    let agent_secret = "testsecret";

    let coordinator_child = spawn_coordinator_with_config(
        coord_port,
        &format!(
            r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts."{agent_id}"]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{agent_secret}"

        [clients."{client_id}"]
        shared_secret = "{client_secret}"
    "#
        ),
    );
    let _coordinator_guard = KillOnDrop(coordinator_child);
    wait_for_listening(coord_port, 5).await;

    // Start the agent first (host is online)
    let _agent_guard = {
        let agent = spawn_host_agent_with_env_args(
            [("SHUTHOST_SHARED_SECRET", agent_secret)].as_slice(),
            [
                "service",
                "--port",
                &agent_port.to_string(),
                "--shutdown-command",
                "",
            ]
            .as_slice(),
        );
        let agent_guard = KillOnDrop(agent);

        // Wait for agent to be listening
        wait_for_agent_ready(agent_port, agent_secret, 5).await;
        agent_guard
    };

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

    // Try to release a lease synchronously when host is online but no lease exists
    let release_url = format!("http://127.0.0.1:{coord_port}/api/m2m/lease/{agent_id}/release",);

    let signed_message = create_signed_message("release", client_secret);

    // Start the release request
    let resp = Client::new()
        .post(&release_url)
        .header("X-Client-ID", client_id)
        .header("X-Request", signed_message)
        .send()
        .await
        .expect("Failed to get resp");

    if resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        panic!(
            "Releasing nonexistent lease succeeded unexpectedly with status {}: {}",
            status, body
        );
    }
}
