//! Integration tests verifying that the coordinator fires webhook notifications
//! for each event kind: `unscheduled` (startup / shutdown), `operation_failed`
//! (startup, shutdown, and the `is_repeat` flag), and `online_for`.
//!
//! Each test spins up a [`crate::common::MockWebhookServer`] that the
//! coordinator's webhook config points to, then triggers the relevant scenario
//! and asserts the exact JSON payload fields.

use core::time::Duration;

use secrecy::SecretString;
use shuthost_coordinator::app::HostState;

use crate::common::{
    MockWebhookServer, get_free_port, runtime_test_config, spawn_coordinator_with_config,
    spawn_host_agent, spawn_host_agent_default, wait_for_agent_ready, wait_for_host_state,
    wait_for_listening,
};

// TODO: add tests for, or harden the existing tests, to detect wrong notifications

// ─────────────────────────────────────────────────────────────────
// unscheduled.startup
// ─────────────────────────────────────────────────────────────────

/// An agent that comes online while no leases are held should fire an
/// `unscheduled { kind: "startup" }` webhook.
#[tokio::test]
async fn webhook_fires_for_unscheduled_startup() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.myhost]
        ip = "127.0.0.1"
        mac = "disableWOL"
        port = {agent_port}
        shared_secret = "{secret}"

        [[notifications.webhooks]]
        url = "{webhook_url}"

        [clients]
        "#,
        webhook_url = webhook.url(),
    ) + &runtime_test_config();

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    // The agent starts up with no leases held — the coordinator should detect
    // an Offline→Online transition without a corresponding lease and fire the
    // unscheduled-startup notification.
    let _agent = spawn_host_agent_default(secret, agent_port);

    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "unscheduled" && p["kind"] == "startup",
            Duration::from_secs(10),
        )
        .await
        .expect("expected unscheduled-startup webhook within timeout");

    assert_eq!(payload["host"], "myhost");
    assert!(payload["at_unix"].is_number());
}

// ─────────────────────────────────────────────────────────────────
// unscheduled.shutdown
// ─────────────────────────────────────────────────────────────────

/// A host that goes offline while a lease is active should fire an
/// `unscheduled { kind: "shutdown" }` webhook.
#[tokio::test]
async fn webhook_fires_for_unscheduled_shutdown() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.myhost]
        ip = "127.0.0.1"
        mac = "disableWOL"
        port = {agent_port}
        shared_secret = "{secret}"

        [[notifications.webhooks]]
        url = "{webhook_url}"

        [clients]
        "#,
        webhook_url = webhook.url(),
    ) + &runtime_test_config();

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    let agent = spawn_host_agent_default(secret, agent_port);

    // Wait until the coordinator sees the host as Online, then take a lease so
    // that a subsequent disappearance is treated as unexpected.
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 15).await,
        "host should become online before taking a lease"
    );

    let client = reqwest::Client::new();
    client
        .post(format!(
            "http://127.0.0.1:{coord_port}/api/lease/myhost/take"
        ))
        .send()
        .await
        .expect("failed to take lease");

    // With disableWOL, the wake attempt returns Noop (WakeErr), setting the
    // actor state to Offline. Wait for the subsequent poll to confirm Online.
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 15).await,
        "host should be online again after WakeErr → poll cycle"
    );

    // Dropping the agent causes the host to go offline while the lease is
    // still held → unscheduled shutdown.
    drop(agent);

    // The predicate ignores any earlier unscheduled-startup payload so the
    // test is insensitive to the ordering of the two events.
    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "unscheduled" && p["kind"] == "shutdown",
            Duration::from_secs(10),
        )
        .await
        .expect("expected unscheduled-shutdown webhook within timeout");

    assert_eq!(payload["host"], "myhost");
    assert!(payload["at_unix"].is_number());
}

// ─────────────────────────────────────────────────────────────────
// operation_failed.startup  (first failure — is_repeat: false)
// ─────────────────────────────────────────────────────────────────

/// When the coordinator tries to wake a host that never comes online, it should
/// fire an `operation_failed { kind: "startup", is_repeat: false }` webhook.
#[tokio::test]
async fn webhook_fires_for_operation_failed_startup() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    // Per-host wake_timeout_secs kept short so the test completes quickly.
    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.myhost]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{secret}"
        wake_timeout_secs = 3

        [[notifications.webhooks]]
        url = "{webhook_url}"
        events = [{{ type = "operation_failed" }}]

        [clients]
        "#,
        webhook_url = webhook.url(),
    ) + &runtime_test_config();

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    // No agent is started — taking a lease triggers a wake attempt that will
    // never succeed, resulting in an operation_failed notification.
    reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{coord_port}/api/lease/myhost/take"
        ))
        .send()
        .await
        .expect("failed to take lease");

    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "operation_failed" && p["kind"] == "startup",
            Duration::from_secs(15),
        )
        .await
        .expect("expected operation_failed-startup webhook within timeout");

    assert_eq!(payload["host"], "myhost");
    assert_eq!(payload["is_repeat"], false);
    assert!(payload["at_unix"].is_number());
}

// ─────────────────────────────────────────────────────────────────
// operation_failed.startup  (retry — is_repeat: true)
// ─────────────────────────────────────────────────────────────────

/// With `enforce_state = true` the coordinator retries the wake after the first
/// failure. The retry should fire a second webhook with `is_repeat: true`.
#[tokio::test]
async fn webhook_fires_for_operation_failed_startup_repeat() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [server.runtime]
        status_poll_interval_secs = 1
        transition_poll_interval_ms = 100
        enforce_stabilization_threshold_secs = 1

        [hosts.myhost]
        ip = "127.0.0.1"
        mac = "00:11:22:33:44:55"
        port = {agent_port}
        shared_secret = "{secret}"
        wake_timeout_secs = 3
        enforce_state = true

        [[notifications.webhooks]]
        url = "{webhook_url}"
        events = [{{ type = "operation_failed" }}]

        [clients]
        "#,
        webhook_url = webhook.url(),
    );

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    // No agent — every wake attempt will time out.
    reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{coord_port}/api/lease/myhost/take"
        ))
        .send()
        .await
        .expect("failed to take lease");

    // First failure: is_repeat must be false.
    let first = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "operation_failed" && p["kind"] == "startup",
            Duration::from_secs(15),
        )
        .await
        .expect("expected first operation_failed-startup webhook");
    assert_eq!(
        first["is_repeat"], false,
        "first failure should not be a repeat"
    );

    // enforce_state re-triggers the wake; the second attempt also fails.
    let second = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "operation_failed" && p["kind"] == "startup",
            Duration::from_secs(15),
        )
        .await
        .expect("expected second operation_failed-startup webhook (repeat)");
    assert_eq!(
        second["is_repeat"], true,
        "second failure should be a repeat"
    );
}

// ─────────────────────────────────────────────────────────────────
// operation_failed.shutdown
// ─────────────────────────────────────────────────────────────────

/// When the coordinator sends a shutdown command but the host never goes
/// offline, it should fire an `operation_failed { kind: "shutdown" }` webhook.
#[tokio::test]
async fn webhook_fires_for_operation_failed_shutdown() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.myhost]
        ip = "127.0.0.1"
        mac = "disableWOL"
        port = {agent_port}
        shared_secret = "{secret}"
        shutdown_timeout_secs = 3

        [[notifications.webhooks]]
        url = "{webhook_url}"
        events = [{{ type = "operation_failed" }}]

        [clients]
        "#,
        webhook_url = webhook.url(),
    ) + &runtime_test_config();

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    // The no-op shutdown command means the agent process stays alive after the
    // coordinator sends the shutdown message, keeping the host Online.
    let _agent = spawn_host_agent(
        secret,
        agent_port,
        shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT,
        "echo noop",
    );
    wait_for_agent_ready(agent_port, &SecretString::from(secret), 5).await;

    let client = reqwest::Client::new();

    // Take a lease so the host is "under coordinator control".
    client
        .post(format!(
            "http://127.0.0.1:{coord_port}/api/lease/myhost/take"
        ))
        .send()
        .await
        .expect("failed to take lease");

    // With disableWOL, the wake attempt is a Noop (WakeErr → Offline). Wait
    // for the poll to confirm the host is Online again before releasing.
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 15).await,
        "host should be online before releasing lease"
    );

    // Releasing the lease triggers a shutdown attempt. The agent ignores it
    // (no-op command) and stays alive, so the coordinator times out.
    client
        .post(format!(
            "http://127.0.0.1:{coord_port}/api/lease/myhost/release"
        ))
        .send()
        .await
        .expect("failed to release lease");

    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "operation_failed" && p["kind"] == "shutdown",
            Duration::from_secs(15),
        )
        .await
        .expect("expected operation_failed-shutdown webhook within timeout");

    assert_eq!(payload["host"], "myhost");
    assert_eq!(payload["is_repeat"], false);
    assert!(payload["at_unix"].is_number());
}

// ─────────────────────────────────────────────────────────────────
// online_for
// ─────────────────────────────────────────────────────────────────

/// When a host has been continuously online for the configured duration, the
/// coordinator should fire an `online_for { online_for_secs }` webhook.
#[tokio::test]
async fn webhook_fires_for_online_for() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    const ONLINE_FOR_SECS: u64 = 3;

    let config = format!(
        r#"
        [server]
        port = {coord_port}
        bind = "127.0.0.1"

        [hosts.myhost]
        ip = "127.0.0.1"
        mac = "disableWOL"
        port = {agent_port}
        shared_secret = "{secret}"

        [[notifications.webhooks]]
        url = "{webhook_url}"
        events = [{{ type = "online_for", duration_secs = {ONLINE_FOR_SECS} }}]

        [clients]
        "#,
        webhook_url = webhook.url(),
    ) + &runtime_test_config();

    let _coord = spawn_coordinator_with_config(coord_port, &config);
    wait_for_listening(coord_port, 5).await;

    let _agent = spawn_host_agent_default(secret, agent_port);

    // Wait for the coordinator to confirm the host is online before starting
    // the clock, then allow enough time for the online_for timer to fire.
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should come online before waiting for online_for webhook"
    );

    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "online_for",
            // online_for_secs (3) + generous buffer for polling and scheduling jitter
            Duration::from_secs(ONLINE_FOR_SECS + 8),
        )
        .await
        .expect("expected online_for webhook within timeout");

    assert_eq!(payload["host"], "myhost");
    assert_eq!(payload["online_for_secs"], ONLINE_FOR_SECS);
    assert!(payload["at_unix"].is_number());
}
