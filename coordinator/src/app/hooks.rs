//! Hook execution for pre-startup and post-shutdown actions.

use core::time::Duration;

use tokio::{process, time::{sleep, timeout}};
use tracing::warn;

use crate::config::{HookAction, HookConfig};

/// Execute a hook action, waiting for it to complete before returning.
///
/// Fail-open: errors are logged as warnings but never propagate. The caller
/// always proceeds regardless of whether the hook succeeded.
pub(crate) async fn run_hook(host_name: &str, label: &str, hook: &HookConfig) {
    let timeout_d = Duration::from_secs(hook.timeout_secs);

    match hook.action {
        HookAction::Exec {
            ref program,
            ref args,
        } => {
            run_exec(host_name, label, program, args, timeout_d).await;
        }
        HookAction::Http {
            ref url,
            ref method,
            ref body,
            delay_secs,
        } => {
            run_http(
                host_name,
                label,
                url,
                method,
                body.as_deref(),
                delay_secs,
                timeout_d,
            )
            .await;
        }
    }
}

#[tracing::instrument(skip(args))]
async fn run_exec(
    host_name: &str,
    label: &str,
    program: &str,
    args: &[String],
    timeout_d: Duration,
) {
    let result = timeout(timeout_d, process::Command::new(program).args(args).output()).await;

    match result {
        Err(_elapsed) => {
            warn!("Hook exec command timed out");
        }
        Ok(Err(e)) => {
            warn!("Hook exec command failed to spawn: {e}");
        }
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!(
                    exit_code = output.status.code(),
                    stderr = %stderr,
                    "Hook exec command exited with non-zero status",
                );
            }
        }
    }
}

#[tracing::instrument(skip(body))]
async fn run_http(
    host_name: &str,
    label: &str,
    url: &reqwest::Url,
    method: &reqwest::Method,
    body: Option<&str>,
    delay_secs: u64,
    timeout_d: Duration,
) {
    if delay_secs > 0 {
        sleep(Duration::from_secs(delay_secs)).await;
    }

    let client = reqwest::Client::new();
    let req_method = method.clone();

    let mut builder = client.request(req_method, url.clone()).timeout(timeout_d);

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
