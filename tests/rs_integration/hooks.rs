//! Integration tests for pre-startup and post-shutdown hooks.
//!
//! Each test spawns a full coordinator configured with a hook, then triggers a
//! host wake via the M2M lease API and verifies the hook fired.  No real host
//! agent is required: `mac = "disableWOL"` suppresses the `WoL` packet, and the
//! lease-take task is left running in the background (it will block waiting for
//! the host to come online, which never happens — the hook has already fired).

use alloc::sync::Arc;
use core::time::Duration;
use std::{env, path::Path};
use tokio::fs;

use axum::{Router, body::Bytes, http::Method, http::StatusCode, routing::any};
use secrecy::SecretString;
use shuthost_common::create_signed_message;
use tokio::{net::TcpListener, sync::Mutex, task, time};

use shuthost_coordinator::app::HostState;

use crate::common::{
    TEST_ENFORCE_THRESHOLD_SECS, get_free_port, runtime_test_config, spawn_coordinator_with_config,
    spawn_host_agent_default, wait_for_agent_ready, wait_for_host_state, wait_for_listening,
};

// ─── Generic mock HTTP server ─────────────────────────────────────────────────

/// Minimal in-process HTTP server that captures the method and raw body of
/// every incoming request on any route and any method.
struct MockHookServer {
    port: u16,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockHookServer {
    async fn start() -> Self {
        let requests: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
        let cap = requests.clone();

        let app = Router::new().route(
            "/",
            any(move |method: Method, body: Bytes| {
                let cap = cap.clone();
                async move {
                    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
                    cap.lock().await.push((method.to_string(), body_str));
                    StatusCode::OK
                }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind failed");
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { port, requests }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// Block until at least one request is captured or `timeout` elapses.
    async fn wait_for_request(&self, timeout: Duration) -> Option<(String, String)> {
        let deadline = time::Instant::now() + timeout;
        loop {
            {
                let mut reqs = self.requests.lock().await;
                if !reqs.is_empty() {
                    return Some(reqs.remove(0));
                }
            }
            if time::Instant::now() >= deadline {
                return None;
            }
            time::sleep(Duration::from_millis(50)).await;
        }
    }
}

// ─── Config helpers ───────────────────────────────────────────────────────────

const CLIENT_ID: &str = "hook-test-client";
const CLIENT_SECRET: &str = "hooktest-secret";

fn base_config(coord_port: u16, agent_port: u16, hook_toml: &str) -> String {
    format!(
        r#"
[server]
port = {coord_port}
bind = "127.0.0.1"

[hosts.myhost]
ip = "127.0.0.1"
mac = "disableWOL"
port = {agent_port}
shared_secret = "hostsecret"

{hook_toml}

[clients."{CLIENT_ID}"]
shared_secret = "{CLIENT_SECRET}"

{rt}
"#,
        rt = runtime_test_config(),
    )
}

/// Spawn a background task that takes a lease on `myhost`.  The task will
/// block until the host comes online (which never happens in these tests) and
/// is implicitly cancelled when the returned handle is dropped.
fn spawn_take_lease(coord_port: u16) -> task::JoinHandle<()> {
    let signed = create_signed_message("take", &SecretString::from(CLIENT_SECRET));
    tokio::spawn(async move {
        drop(
            reqwest::Client::new()
                .post(format!(
                    "http://127.0.0.1:{coord_port}/api/m2m/lease/myhost/take"
                ))
                .header("X-Client-ID", CLIENT_ID)
                .header("X-Request", signed)
                .send()
                .await,
        );
    })
}

// ── HTTP hook tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn http_pre_startup_hook_get_fires_on_wake() {
    let coord_port = get_free_port();
    let hook_server = MockHookServer::start().await;

    let _coord = spawn_coordinator_with_config(
        coord_port,
        &base_config(
            coord_port,
            get_free_port(),
            &format!(
                r#"[hosts.myhost.pre_startup]
type = "http"
url = "{}"
method = "GET"
timeout_secs = 5"#,
                hook_server.url()
            ),
        ),
    );
    wait_for_listening(coord_port, 5).await;

    let _task = spawn_take_lease(coord_port);

    let (method, body) = hook_server
        .wait_for_request(Duration::from_secs(10))
        .await
        .expect("pre_startup HTTP hook should have fired");
    assert_eq!(method, "GET");
    assert_eq!(body, "");
}

#[tokio::test]
async fn http_pre_startup_hook_post_with_body_fires_on_wake() {
    let coord_port = get_free_port();
    let hook_server = MockHookServer::start().await;

    let _coord = spawn_coordinator_with_config(
        coord_port,
        &base_config(
            coord_port,
            get_free_port(),
            &format!(
                r#"[hosts.myhost.pre_startup]
type = "http"
url = "{}"
method = "POST"
body = '{{"on": true}}'
timeout_secs = 5"#,
                hook_server.url()
            ),
        ),
    );
    wait_for_listening(coord_port, 5).await;

    let _task = spawn_take_lease(coord_port);

    let (method, body) = hook_server
        .wait_for_request(Duration::from_secs(10))
        .await
        .expect("pre_startup HTTP hook should have fired");
    assert_eq!(method, "POST");
    assert_eq!(body, r#"{"on": true}"#);
}

// ── Shell hook tests (Unix only: hooks.rs uses `sh -c`) ──────────────────────

/// Wait for `path` to appear and contain `expected_content` (trimmed), or panic on timeout.
#[cfg(unix)]
async fn wait_for_file_content(path: &Path, expected_content: &str, timeout: Duration) {
    let deadline = time::Instant::now() + timeout;
    loop {
        if let Ok(content) = fs::read_to_string(path).await
            && content.trim() == expected_content
        {
            return;
        }
        assert!(
            time::Instant::now() < deadline,
            "hook signal file {path:?} did not appear with expected content within timeout",
        );
        time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn shell_pre_startup_hook_fires_on_wake() {
    let coord_port = get_free_port();
    let signal_file = env::temp_dir().join(format!("shuthost_hook_test_{coord_port}"));
    drop(fs::remove_file(&signal_file).await);
    let signal_path = signal_file.display().to_string();

    let _coord = spawn_coordinator_with_config(
        coord_port,
        &base_config(
            coord_port,
            get_free_port(),
            &format!(
                r#"[hosts.myhost.pre_startup]
type = "shell"
command = "echo done > {signal_path}"
timeout_secs = 5"#
            ),
        ),
    );
    wait_for_listening(coord_port, 5).await;

    let _task = spawn_take_lease(coord_port);

    wait_for_file_content(&signal_file, "done", Duration::from_secs(10)).await;
    drop(fs::remove_file(&signal_file).await);
}

// ── post_shutdown hook tests ───────────────────────────────────────────────────

/// Coordinator config for `post_shutdown` tests: a real agent can connect,
/// `enforce_state = true` so the coordinator shuts the host down once no lease
/// holds it online, and the given hook TOML fragment is appended.
fn agent_config(coord_port: u16, agent_port: u16, hook_toml: &str) -> String {
    format!(
        r#"
[server]
port = {coord_port}
bind = "127.0.0.1"

[hosts.myhost]
ip = "127.0.0.1"
mac = "00:11:22:33:44:55"
port = {agent_port}
shared_secret = "hostsecret"
enforce_state = true

{hook_toml}

[clients]

{rt}
"#,
        rt = runtime_test_config(),
    )
}

#[tokio::test]
async fn http_post_shutdown_hook_fires_after_shutdown() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let hook_server = MockHookServer::start().await;

    let _coord = spawn_coordinator_with_config(
        coord_port,
        &agent_config(
            coord_port,
            agent_port,
            &format!(
                r#"[hosts.myhost.post_shutdown]
type = "http"
url = "{}"
method = "POST"
timeout_secs = 5"#,
                hook_server.url()
            ),
        ),
    );
    wait_for_listening(coord_port, 5).await;

    let agent = spawn_host_agent_default("hostsecret", agent_port);
    wait_for_agent_ready(agent_port, &SecretString::from("hostsecret"), 5).await;

    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should be online before enforcer fires"
    );

    // Wait until the enforcer has fired and `shutdown_host_and_wait` is polling.
    time::sleep(Duration::from_secs(TEST_ENFORCE_THRESHOLD_SECS + 2)).await;

    // Dropping the agent sends Abort (stops agent thread).  The coordinator's
    // next poll detects Offline, completes the shutdown sequence, and fires the
    // post_shutdown hook.
    drop(agent);

    let (method, body) = hook_server
        .wait_for_request(Duration::from_secs(10))
        .await
        .expect("post_shutdown HTTP hook should have fired");
    assert_eq!(method, "POST");
    assert_eq!(body, "");
}

#[cfg(unix)]
#[tokio::test]
async fn shell_post_shutdown_hook_fires_after_shutdown() {
    let coord_port = get_free_port();
    let agent_port = get_free_port();
    let signal_file = env::temp_dir().join(format!("shuthost_post_shutdown_hook_{coord_port}"));
    drop(fs::remove_file(&signal_file));
    let signal_path = signal_file.display().to_string();

    let _coord = spawn_coordinator_with_config(
        coord_port,
        &agent_config(
            coord_port,
            agent_port,
            &format!(
                r#"[hosts.myhost.post_shutdown]
type = "shell"
command = "echo done > {signal_path}"
timeout_secs = 5"#
            ),
        ),
    );
    wait_for_listening(coord_port, 5).await;

    let agent = spawn_host_agent_default("hostsecret", agent_port);
    wait_for_agent_ready(agent_port, &SecretString::from("hostsecret"), 5).await;

    assert!(
        wait_for_host_state(coord_port, "myhost", HostState::Online, 10).await,
        "host should be online before enforcer fires"
    );

    // Wait until the enforcer has fired and `shutdown_host_and_wait` is polling.
    time::sleep(Duration::from_secs(TEST_ENFORCE_THRESHOLD_SECS + 2)).await;

    drop(agent);

    wait_for_file_content(&signal_file, "done", Duration::from_secs(10)).await;
    drop(fs::remove_file(&signal_file));
}
