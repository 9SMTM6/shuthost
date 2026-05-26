//! Centralised notification dispatch: web-push (PWA) and config-driven webhooks.
//!
//! All notification callsites in the coordinator (unscheduled state changes,
//! operation failures, online-for timers) funnel through [`dispatch`], which
//! reads the current webhook configuration from the hot-reloadable [`ConfigRx`]
//! and also forwards to the existing PWA push infrastructure.

use alloc::sync::Arc;
use std::time::SystemTime;

use secrecy::ExposeSecret as _;
use serde::Serialize;
use tracing::{error, warn};
use web_push_native::jwt_simple::algorithms::ES256KeyPair;

use crate::{
    app::{db, state::OperationKind},
    config::{SimpleEventFilter, StructuredEventFilter, WebhookConfig, WebhookEventFilter},
    http::push,
};

// ─────────────────────────────────────────────────────────────────
// Public event type
// ─────────────────────────────────────────────────────────────────

/// A notification event produced by the coordinator runtime.
pub(crate) struct NotificationEvent {
    pub(crate) host: String,
    pub(crate) kind: EventKind,
}

/// The kind of notification event.
///
/// This enum is also serialized directly as the webhook payload event object
/// (tagged by `"event"`) via [`WebhookPayload`].
#[derive(Serialize, Clone, Copy)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum EventKind {
    /// An unscheduled host state change — `kind: Startup` means the host came online
    /// without coordinator involvement; `kind: Shutdown` means it went offline
    /// while leases were held.
    Unscheduled {
        kind: OperationKind,
    },
    /// `is_repeat` is `true` when the same failure kind was already recorded for
    /// this host (i.e. a previous enforce-state retry). Webhooks always fire;
    /// PWA push is suppressed for repeats.
    OperationFailed {
        kind: OperationKind,
        is_repeat: bool,
    },
    OnlineFor {
        online_for_secs: u64,
    },
}

// ─────────────────────────────────────────────────────────────────
// Webhook payload types
// ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct WebhookPayload {
    host: String,
    at_unix: u64,
    #[serde(flatten)]
    event: EventKind,
}

// ─────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────

/// Dispatch a notification event to all configured channels.
///
/// - Fires matching webhooks from the current (hot-reloaded) config snapshot.
/// - Forwards to PWA web-push when `pool` and `vapid_key` are available
///   (skips push for repeated operation failures).
pub(crate) async fn dispatch(
    event: NotificationEvent,
    webhooks: &[WebhookConfig],
    pool: Option<&db::DbPool>,
    vapid_key: Option<&Arc<ES256KeyPair>>,
) {
    fire_matching_webhooks(&event, webhooks).await;
    fire_push_notifications(event, pool, vapid_key).await;
}

// ─────────────────────────────────────────────────────────────────
// Webhook dispatch
// ─────────────────────────────────────────────────────────────────

async fn fire_matching_webhooks(event: &NotificationEvent, webhooks: &[WebhookConfig]) {
    let at_unix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for webhook in webhooks {
        if filter_matches(event, webhook) {
            let payload = build_payload(event, at_unix);
            send_webhook(webhook, payload).await;
        }
    }
}

/// Returns `true` if `webhook` should fire for `event`.
fn filter_matches(event: &NotificationEvent, webhook: &WebhookConfig) -> bool {
    match webhook.events {
        // Absent = default: all-host unscheduled + operation_failed; never online_for.
        None => matches!(
            event.kind,
            EventKind::Unscheduled { .. } | EventKind::OperationFailed { .. }
        ),
        // Explicit filter list: fire if any filter matches.
        Some(ref filters) => filters.iter().any(|f| filter_entry_matches(f, event)),
    }
}

fn filter_entry_matches(filter: &WebhookEventFilter, event: &NotificationEvent) -> bool {
    match *filter {
        WebhookEventFilter::Simple(simple) => match simple {
            SimpleEventFilter::Unscheduled => {
                matches!(event.kind, EventKind::Unscheduled { .. })
            }
            SimpleEventFilter::OperationFailed => {
                matches!(event.kind, EventKind::OperationFailed { .. })
            }
        },
        WebhookEventFilter::Structured(ref structured) => match *structured {
            StructuredEventFilter::Unscheduled { ref hosts } => {
                matches!(event.kind, EventKind::Unscheduled { .. })
                    && host_matches(&event.host, hosts.as_ref())
            }
            StructuredEventFilter::OperationFailed { ref hosts } => {
                matches!(event.kind, EventKind::OperationFailed { .. })
                    && host_matches(&event.host, hosts.as_ref())
            }
            StructuredEventFilter::OnlineFor {
                duration_secs,
                ref hosts,
            } => {
                matches!(
                    event.kind,
                    EventKind::OnlineFor { online_for_secs: d } if d == duration_secs
                ) && host_matches(&event.host, hosts.as_ref())
            }
        },
    }
}

/// Returns `true` when `host` is included in the filter's host list.
///
/// - `None` (absent) = all hosts
/// - `Some([])` = no hosts
/// - `Some([...])` = membership check
fn host_matches(host: &str, hosts: Option<&Vec<String>>) -> bool {
    match hosts {
        None => true,
        Some(list) => list.iter().any(|h| h == host),
    }
}

fn build_payload(event: &NotificationEvent, at_unix: u64) -> WebhookPayload {
    WebhookPayload {
        host: event.host.clone(),
        at_unix,
        event: event.kind,
    }
}

async fn send_webhook(webhook: &WebhookConfig, payload: WebhookPayload) {
    let body = serde_json::to_string(&payload).expect("WebhookPayload serialization must not fail");

    let client = reqwest::Client::new();
    let mut req = client
        .post(&webhook.url)
        .header("Content-Type", "application/json");

    // Attach configured headers (e.g. Authorization).
    for (name, value) in &webhook.headers {
        req = req.header(name.as_str(), value.expose_secret());
    }

    // Attach HMAC-SHA256 signature header if a secret is configured.
    if let Some(ref secret) = webhook.secret {
        let sig = shuthost_common::sign_hmac(&body, secret);
        req = req.header("X-ShutHost-Signature", format!("sha256={sig}"));
    }

    match req.body(body).send().await {
        Ok(resp) if resp.status().is_success() => {}
        Ok(resp) => {
            warn!(url = %webhook.url, status = %resp.status(), "Webhook POST failed");
        }
        Err(e) => {
            warn!(url = %webhook.url, "Webhook POST error: {e}");
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// PWA push dispatch
// ─────────────────────────────────────────────────────────────────

async fn fire_push_notifications(
    event: NotificationEvent,
    pool: Option<&db::DbPool>,
    vapid_key: Option<&Arc<ES256KeyPair>>,
) {
    let (Some(pool), Some(vapid_key)) = (pool, vapid_key) else {
        return;
    };

    let NotificationEvent { host, kind } = event;
    match kind {
        EventKind::Unscheduled { kind } => {
            let body = match kind {
                OperationKind::Startup => format!("{host} started up unexpectedly"),
                OperationKind::Shutdown => format!("{host} shut down unexpectedly"),
            };
            match db::get_subscriptions_for_host_unscheduled(pool, &host).await {
                Ok(subs) if !subs.is_empty() => {
                    let payload = push::NotificationPayload::with_data(
                        body,
                        push::HostSpecificNotificationData {
                            hostname: host.clone(),
                        },
                    )
                    .into_json();
                    push::send_push_notifications(vapid_key, pool, &subs, &payload).await;
                }
                Ok(_) => {}
                Err(e) => {
                    error!(host = %host, "Failed to fetch unscheduled push subscriptions: {e:#}");
                }
            }
        }
        EventKind::OperationFailed {
            kind: operation,
            is_repeat,
        } => {
            // Suppress repeated push notifications — the first one already alerted the user.
            if is_repeat {
                return;
            }
            let body = match operation {
                OperationKind::Shutdown => format!("{host} failed to shut down"),
                OperationKind::Startup => format!("{host} failed to start up"),
            };
            match db::get_subscriptions_for_host_operation_failed(pool, &host).await {
                Ok(subs) if !subs.is_empty() => {
                    let payload = push::NotificationPayload::with_data(
                        body,
                        push::HostSpecificNotificationData {
                            hostname: host.clone(),
                        },
                    )
                    .into_json();
                    push::send_push_notifications(vapid_key, pool, &subs, &payload).await;
                }
                Ok(_) => {}
                Err(e) => {
                    error!(host = %host, "Failed to fetch operation-failed push subscriptions: {e:#}");
                }
            }
        }
        // PWA online-for notifications are driven by individual timer tasks in
        // spawn_online_for_notifications; they are not dispatched through here.
        EventKind::OnlineFor { .. } => {}
    }
}
