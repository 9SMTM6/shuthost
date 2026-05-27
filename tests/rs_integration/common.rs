//! Common utilities for integration tests.
//!
//! This module provides shared functions and types used across multiple integration test modules,
//! such as spawning processes, managing ports, and waiting for services to be ready.
#![cfg_attr(
    coverage,
    expect(
        unreachable_patterns,
        reason = "For some reason clippy sets coverage cfg?"
    )
)]

use alloc::sync::Arc;
use core::time::Duration;
use std::{
    env, fs,
    io::Write as _,
    net::{TcpListener as StdTcpListener, TcpStream as StdTcpStream},
    path::Path,
    thread,
    time::Instant,
};

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use clap::Parser as _;
use secrecy::SecretString;
use shuthost_common::CoordinatorMessage;
use shuthost_coordinator::cli::Cli as CoordinatorCli;
use shuthost_host_agent::Cli as AgentCli;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    task,
    time::{self, timeout},
};

use shuthost_coordinator::app::{HostState, HostStatus};

pub(crate) const fn host_agent_bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_host_agent")
}

/// Enforce-state stabilization threshold used in tests. Kept short so the
/// `enforce_state` integration tests complete quickly.
pub(crate) const TEST_ENFORCE_THRESHOLD_SECS: u64 = 2;

/// Returns a `[server.runtime]` TOML block with shortened timeouts/intervals
/// suitable for integration tests. Paste this into coordinator config strings.
pub(crate) fn runtime_test_config() -> String {
    format!(
        "
[server.runtime]
status_poll_interval_secs = 1
transition_poll_interval_ms = 100
enforce_stabilization_threshold_secs = {TEST_ENFORCE_THRESHOLD_SECS}
"
    )
}

pub(crate) fn get_free_port() -> u16 {
    // Bind to port 0 to let the OS pick a free port, then release it and return
    // the port number for the coordinator/agent to bind to.
    //
    // TOCTOU note: there is a small window between dropping the listener here
    // and the coordinator/agent binding the port where another process could
    // steal it. In practice this is extremely unlikely because:
    //
    //   1. Modern OS kernels (Linux, macOS, Windows) avoid reusing recently
    //      released ephemeral ports for a short period via a "TIME_WAIT" or
    //      similar port reuse avoidance mechanism.
    //   2. The ports assigned by `bind(0)` are unpredictable, so there is no
    //      systematic collision between concurrent tests.
    //   3. The coordinator/agent starts binding almost immediately after this
    //      call returns.
    //
    // The alternative — passing a pre-bound `TcpListener` into the
    // coordinator/agent — would eliminate the race entirely but requires
    // production code changes. The current approach is the accepted industry
    // standard for test port allocation.
    StdTcpListener::bind("127.0.0.1:0")
        .expect("failed to bind to port 0")
        .local_addr()
        .expect("failed to get local addr")
        .port()
}

/// Guard that kills the coordinator or agent when dropped.
pub(crate) enum KillOnDrop {
    Coordinator(task::JoinHandle<()>),
    Agent {
        thread: Option<thread::JoinHandle<()>>,
        port: u16,
        secret: SecretString,
    },
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        match *self {
            KillOnDrop::Coordinator(ref handle) => {
                handle.abort();
            }
            KillOnDrop::Agent {
                ref mut thread,
                port,
                ref secret,
            } => {
                // Send abort command to the agent
                if let Ok(mut stream) = StdTcpStream::connect(("127.0.0.1", port)) {
                    let signed_message = shuthost_common::create_signed_message(
                        &CoordinatorMessage::Abort.to_string(),
                        secret,
                    );
                    drop(stream.write_all(signed_message.as_bytes()));
                }
                if let Some(handle) = thread.take() {
                    drop(handle.join());
                }
            }
        }
    }
}

/// Spawn the coordinator service from a given config string.
/// Writes the config to a temp file and spawns the coordinator binary.
pub(crate) fn spawn_coordinator_with_config(port: u16, config_toml: &str) -> KillOnDrop {
    let tmp = env::temp_dir().join(format!("integration_test_config_{port}.toml"));
    fs::write(&tmp, config_toml).expect("failed to write config");

    spawn_coordinator_with_config_file(&tmp, port)
}

/// Spawn the coordinator service from a given config file path.
pub(crate) fn spawn_coordinator_with_config_file(
    config_path: &Path,
    broadcast_port: u16,
) -> KillOnDrop {
    let cli = CoordinatorCli::parse_from([
        "shuthost_coordinator",
        "control-service",
        "--log-format",
        "pretty",
        "--config",
        config_path.to_str().unwrap(),
        "--broadcast-port",
        &broadcast_port.to_string(),
    ]);
    let handle = tokio::spawn(async move {
        // SAFETY: This is only used in integration tests and no user-facing code. It just tells the coordinator to log less verbose output.
        unsafe {
            env::set_var("SHUTHOST_INTEGRATION_TEST", "1");
        }
        shuthost_coordinator::inner_main(cli)
            .await
            .expect("inner_main failed");
    });
    KillOnDrop::Coordinator(handle)
}

/// Spawn the host agent in a separate thread with the given secret, listen port,
/// broadcast port, and shutdown command.
pub(crate) fn spawn_host_agent(
    secret: &str,
    port: u16,
    broadcast_port: u16,
    shutdown_command: &str,
) -> KillOnDrop {
    let cli = AgentCli::parse_from([
        "shuthost_host_agent",
        "service",
        "--port",
        &port.to_string(),
        "--broadcast-port",
        &broadcast_port.to_string(),
        "--shutdown-command",
        shutdown_command,
    ]);
    let shuthost_host_agent::Command::Service(mut config) = cli.command else {
        panic!("Expected service command")
    };
    config.shared_secret = Some(SecretString::from(secret));
    let new_cli = AgentCli {
        command: shuthost_host_agent::Command::Service(config),
    };
    let handle = thread::spawn(move || {
        shuthost_host_agent::inner_main(new_cli);
    });
    KillOnDrop::Agent {
        thread: Some(handle),
        port,
        secret: SecretString::from(secret),
    }
}

/// Spawn a test host agent with the given secret and port.
pub(crate) fn spawn_host_agent_default(secret: &str, port: u16) -> KillOnDrop {
    spawn_host_agent(secret, port, port, "")
}

/// Block until a TCP listener is accepting on `127.0.0.1:port` or timeout.
pub(crate) async fn wait_for_listening(port: u16, timeout_secs: u64) {
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).await.is_err() {
        assert!(
            start.elapsed() <= Duration::from_secs(timeout_secs),
            "server did not start within timeout"
        );
        time::sleep(Duration::from_millis(100)).await;
    }
}

/// Block until the host agent is ready to accept status requests.
/// Sends a proper HMAC-signed status message to verify the agent is responding correctly.
pub(crate) async fn wait_for_agent_ready(
    port: u16,
    shared_secret: &SecretString,
    timeout_secs: u64,
) {
    let start = Instant::now();
    let addr = format!("127.0.0.1:{port}");

    while start.elapsed() < Duration::from_secs(timeout_secs) {
        if let Ok(Ok(mut stream)) =
            timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await
        {
            // Send a proper status request like the coordinator does
            let signed_message = shuthost_common::create_signed_message("status", shared_secret);
            if stream.write_all(signed_message.as_bytes()).await.is_err() {
                time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let mut buf = vec![0u8; 256];
            match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    let data = &buf[..n];
                    let resp = String::from_utf8_lossy(data);
                    // Accept any non-error response as ready
                    if !resp.contains("ERROR") {
                        return;
                    }
                }
                _ => {}
            }
        }
        time::sleep(Duration::from_millis(100)).await;
    }
    panic!("agent on port {port} did not become ready within timeout");
}

/// Wait until the coordinator reports the specified host in the expected state.
/// Polls the /`api/hosts_status` endpoint until the host reaches the desired state or timeout.
pub(crate) async fn wait_for_host_state(
    coord_port: u16,
    host_name: &str,
    expected_state: HostState,
    max_attempts: usize,
) -> bool {
    let client = reqwest::Client::new();
    let status_url = format!("http://127.0.0.1:{coord_port}/api/hosts_status");

    for _ in 0..max_attempts {
        let resp = client.get(&status_url).send().await;
        if let Ok(resp) = resp
            && let Ok(json) = resp.json::<HostStatus>().await
            && json.get(host_name) == Some(&expected_state)
        {
            return true;
        }
        time::sleep(Duration::from_millis(300)).await;
    }
    false
}

/// A minimal in-process HTTP server that collects JSON webhook payloads `POSTed` to it.
///
/// Used by notification integration tests to verify that the coordinator fires
/// the correct webhook payload for each event kind.
pub(crate) struct MockWebhookServer {
    port: u16,
    payloads: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl MockWebhookServer {
    pub(crate) async fn start() -> Self {
        let payloads: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let payloads_task = payloads.clone();

        let app = Router::new().route(
            "/",
            post(move |body: Bytes| {
                let payloads = payloads_task.clone();
                async move {
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) {
                        payloads.lock().await.push(v);
                    }
                    StatusCode::OK
                }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind mock webhook server");
        let port = listener.local_addr().expect("no local addr").port();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { port, payloads }
    }

    pub(crate) fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Drains and returns all payloads received so far, leaving the queue empty.
    ///
    /// Useful for negative assertions: sleep a settling window, then call this
    /// and assert that no unexpected payloads accumulated.
    pub(crate) async fn drain_all_payloads(&self) -> Vec<serde_json::Value> {
        std::mem::take(&mut *self.payloads.lock().await)
    }

    /// Poll until a payload satisfying `predicate` arrives, or `timeout` elapses.
    ///
    /// Non-matching payloads are left in the queue so that ordering between
    /// different event kinds does not matter (e.g. an `unscheduled.startup`
    /// that fires before the `unscheduled.shutdown` we actually care about).
    pub(crate) async fn wait_for_matching_payload<F>(
        &self,
        predicate: F,
        timeout: Duration,
    ) -> Option<serde_json::Value>
    where
        F: Fn(&serde_json::Value) -> bool,
    {
        let deadline = time::Instant::now() + timeout;
        loop {
            {
                let mut payloads = self.payloads.lock().await;
                if let Some(pos) = payloads.iter().position(&predicate) {
                    return Some(payloads.remove(pos));
                }
            }
            if time::Instant::now() >= deadline {
                return None;
            }
            time::sleep(Duration::from_millis(100)).await;
        }
    }
}
