//! Hook execution for pre-startup and post-shutdown actions.

use core::time::Duration;

use tokio::{process, time::timeout};
use tracing::warn;

use crate::config::{HookAction, HookConfig};

/// Default timeout in seconds for hook execution when not specified in config.
const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 30;

/// Execute a hook action, waiting for it to complete before returning.
///
/// Fail-open: errors are logged as warnings but never propagate. The caller
/// always proceeds regardless of whether the hook succeeded.
pub(crate) async fn run_hook(host_name: &str, label: &str, hook: &HookConfig) {
    let timeout = Duration::from_secs(hook.timeout_secs.unwrap_or(DEFAULT_HOOK_TIMEOUT_SECS));

    match hook.action {
        HookAction::Shell { ref command } => {
            run_shell(host_name, label, command, timeout).await;
        }
        HookAction::Http {
            ref url,
            ref method,
            ref body,
        } => {
            run_http(host_name, label, url, method, body.as_deref(), timeout).await;
        }
    }
}

#[tracing::instrument]
async fn run_shell(host_name: &str, label: &str, command: &str, duration: Duration) {
    let result = timeout(
        duration,
        process::Command::new("sh").arg("-c").arg(command).output(),
    )
    .await;

    match result {
        Err(_elapsed) => {
            warn!("Hook shell command timed out");
        }
        Ok(Err(e)) => {
            warn!("Hook shell command failed to spawn: {e}");
        }
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!(
                    exit_code = output.status.code(),
                    stderr = %stderr,
                    "Hook shell command exited with non-zero status",
                );
            }
        }
    }
}

#[tracing::instrument]
async fn run_http(
    host_name: &str,
    label: &str,
    url: &reqwest::Url,
    method: &reqwest::Method,
    body: Option<&str>,
    timeout: Duration,
) {
    let client = reqwest::Client::new();
    let req_method = method.clone();

    let mut builder = client.request(req_method, url.clone()).timeout(timeout);

    if let Some(b) = body {
        builder = builder.body(b.to_owned());
    }

    match builder.send().await {
        Ok(resp) if resp.status().is_success() => {}
        Ok(resp) => {
            warn!(
                status = %resp.status(),
                "Hook HTTP request returned non-success status",
            );
        }
        Err(e) => {
            warn!("Hook HTTP request failed: {e}");
        }
    }
}
