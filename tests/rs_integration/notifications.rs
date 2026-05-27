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
use tokio::time::sleep;

use crate::common::{
    MockWebhookServer, get_free_port, runtime_test_config, spawn_coordinator_with_config,
    spawn_host_agent, spawn_host_agent_default, wait_for_agent_ready, wait_for_host_state,
    wait_for_listening,
};

/// Short sleep after a successful assertion to let any spurious follow-on
/// notifications accumulate before we drain and check for unexpected payloads.
/// Two poll cycles (2 × `status_poll_interval_secs = 1 s`) is sufficient.
const SETTLING_SECS: u64 = 2;

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

    // After the startup notification, allow two poll cycles to elapse and verify
    // that no spurious `unscheduled.shutdown` (or any other wrong event) was sent.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let unexpected = webhook.drain_all_payloads().await;
    assert!(
        !unexpected
            .iter()
            .any(|p| p["event"] == "unscheduled" && p["kind"] == "shutdown"),
        "unexpected unscheduled-shutdown notification after startup: {unexpected:?}"
    );
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

    // Drain the queue now so any startup notification from the WakeErr→Offline→
    // Online cycle does not pollute the shutdown assertion below.
    webhook.drain_all_payloads().await;

    // Dropping the agent causes the host to go offline while the lease is
    // still held → unscheduled shutdown.
    drop(agent);

    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "unscheduled" && p["kind"] == "shutdown",
            Duration::from_secs(10),
        )
        .await
        .expect("expected unscheduled-shutdown webhook within timeout");

    assert_eq!(payload["host"], "myhost");
    assert!(payload["at_unix"].is_number());

    // Agent is gone; allow settling time and verify the queue is fully empty.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let unexpected = webhook.drain_all_payloads().await;
    assert!(
        unexpected.is_empty(),
        "unexpected extra payloads after unscheduled-shutdown: {unexpected:?}"
    );
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

    // No agent, no enforce_state — no retry is scheduled. Verify silence.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let unexpected = webhook.drain_all_payloads().await;
    assert!(
        unexpected.is_empty(),
        "unexpected extra payloads after operation_failed-startup: {unexpected:?}"
    );
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

    // enforce_state keeps retrying, so further operation_failed.startup payloads
    // are expected. Only assert that no wrong-kind events slipped through.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let extra = webhook.drain_all_payloads().await;
    assert!(
        extra
            .iter()
            .all(|p| p["event"] == "operation_failed" && p["kind"] == "startup"),
        "unexpected wrong-kind payloads after repeated operation_failed-startup: {extra:?}"
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

    // Without enforce_state, no retry is scheduled. Verify silence.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let unexpected = webhook.drain_all_payloads().await;
    assert!(
        unexpected.is_empty(),
        "unexpected extra payloads after operation_failed-shutdown: {unexpected:?}"
    );
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

    // online_for fires once per online session; the host is still up, so no
    // further notifications should arrive during the settling window.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let unexpected = webhook.drain_all_payloads().await;
    assert!(
        unexpected.is_empty(),
        "unexpected extra payloads after online_for: {unexpected:?}"
    );
}

// ─────────────────────────────────────────────────────────────────
// Negative tests — events that must NOT fire
// ─────────────────────────────────────────────────────────────────

/// A host that goes offline while **no** leases are held must not produce an
/// `unscheduled.shutdown` notification (graceful / unmanaged shutdown).
#[tokio::test]
async fn no_unscheduled_shutdown_for_graceful_offline() {
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
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should come online"
    );

    // Drain any startup notification so it does not pollute the shutdown check.
    webhook.drain_all_payloads().await;

    // Drop the agent — host goes Offline with no active leases.  This
    // transition must NOT produce an `unscheduled.shutdown` notification.
    drop(agent);

    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let payloads = webhook.drain_all_payloads().await;
    assert!(
        !payloads
            .iter()
            .any(|p| p["event"] == "unscheduled" && p["kind"] == "shutdown"),
        "unexpected unscheduled-shutdown notification for graceful offline: {payloads:?}"
    );
}

/// A host that comes online while a lease is already held must not fire an
/// `unscheduled.startup` notification.
#[tokio::test]
async fn no_unscheduled_startup_when_lease_held_at_online() {
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

    // Take a lease BEFORE the agent starts.  The coordinator attempts a wake
    // (WakeErr with disableWOL) and records the host as Offline.  When the
    // agent then comes online the Offline→Online transition must NOT fire
    // `unscheduled.startup` because a lease is held.
    reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{coord_port}/api/lease/myhost/take"
        ))
        .send()
        .await
        .expect("failed to take lease");

    let _agent = spawn_host_agent_default(secret, agent_port);
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 15).await,
        "host should come online after taking lease"
    );

    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let payloads = webhook.drain_all_payloads().await;
    assert!(
        !payloads
            .iter()
            .any(|p| p["event"] == "unscheduled" && p["kind"] == "startup"),
        "unexpected unscheduled-startup with active lease: {payloads:?}"
    );
}

/// An `online_for` notification must not fire when the host goes offline before
/// the configured duration has elapsed.
#[tokio::test]
async fn no_online_for_when_host_goes_offline_before_threshold() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let webhook = MockWebhookServer::start().await;
    let secret = "testsecret";

    const ONLINE_FOR_SECS: u64 = 5;

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

    let agent = spawn_host_agent_default(secret, agent_port);
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should come online"
    );

    // Drop the agent well before the threshold (half the duration).
    sleep(Duration::from_secs(ONLINE_FOR_SECS / 2)).await;
    drop(agent);

    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Offline, 10).await,
        "host should go offline after agent drop"
    );

    // Wait out the remaining original threshold plus a settling buffer to
    // confirm the timer was cancelled when the host went offline.
    sleep(Duration::from_secs(ONLINE_FOR_SECS / 2 + SETTLING_SECS)).await;
    let payloads = webhook.drain_all_payloads().await;
    assert!(
        payloads.is_empty(),
        "online_for must not fire when host went offline before threshold: {payloads:?}"
    );
}

/// When a host goes offline and then comes back online the `online_for` timer
/// must restart from zero.  Exactly one notification should fire (for the
/// second session); the partial uptime of the first session must not carry over.
#[tokio::test]
async fn online_for_timer_resets_on_offline_and_online_again() {
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

    // Session 1: agent comes online, then drops before the threshold.
    let agent = spawn_host_agent_default(secret, agent_port);
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should come online (session 1)"
    );
    sleep(Duration::from_secs(1)).await;
    drop(agent);
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Offline, 10).await,
        "host should go offline after session 1 agent drop"
    );

    // Session 2: restart the agent — the timer must restart from zero.
    let _agent2 = spawn_host_agent_default(secret, agent_port);
    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should come online (session 2)"
    );

    // Wait for the notification; it must arrive after a full threshold elapses
    // from the start of session 2 (not from any earlier accumulated time).
    let payload = webhook
        .wait_for_matching_payload(
            |p| p["event"] == "online_for",
            Duration::from_secs(ONLINE_FOR_SECS + 8),
        )
        .await
        .expect("expected online_for webhook for second online session");

    assert_eq!(payload["online_for_secs"], ONLINE_FOR_SECS);
    assert_eq!(payload["host"], "myhost");

    // Settle and verify no duplicate notification arrived.
    sleep(Duration::from_secs(SETTLING_SECS)).await;
    let extra = webhook.drain_all_payloads().await;
    assert!(
        extra.is_empty(),
        "only one online_for notification expected, got extra: {extra:?}"
    );
}
