//! Host control application logic (non-HTTP). This module contains the core
//! operations for waking/shutting hosts and polling their state.

use alloc::sync::Arc;
use core::{ops, time::Duration};
use std::collections::{HashMap, HashSet};

use eyre::{Context as _, Report};
use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;
#[cfg(not(any(coverage, test)))]
use tokio::time::{MissedTickBehavior, interval};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::{Instant, timeout_at},
};
use tracing::{Instrument as _, debug, info};

use crate::app::{
    AppState, HostStatusStore, OperationFailure, OperationKind, db,
    runtime::{PollError, poll_until_host_state},
    shared_watch_store::{SharedWatchRx, SharedWatchStore},
    state::HostState,
};
use crate::http::push;

use crate::config::{Host, RuntimeConfig};
#[cfg(not(any(coverage, test)))]
use crate::wol;

/// Combines a host name with its `Host` configuration.
#[derive(Debug, Clone)]
pub(crate) struct HostWithName {
    /// Logical name/identifier of the host as present in the config map.
    pub name: String,
    /// The host configuration data.
    pub host: Host,
}

/// A [`HostWithName`] that has had runtime IP/port overrides applied.
///
/// The private field prevents construction outside of this module; the only
/// way to obtain a `ResolvedHost` is via [`lookup_host_with_overrides`], which
/// guarantees the overrides have been applied.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedHost(HostWithName);

impl ops::Deref for ResolvedHost {
    type Target = HostWithName;
    fn deref(&self) -> &HostWithName {
        &self.0
    }
}

/// The set of lease sources for a single host
pub(crate) type LeaseSources = HashSet<LeaseSource>;

/// `host_name` => set of lease sources holding lease
pub(crate) type LeaseMapRaw = HashMap<String, LeaseSources>;

pub(crate) type LeaseStore = SharedWatchStore<LeaseMapRaw>;
pub(crate) type LeaseRx = SharedWatchRx<LeaseMapRaw>;

impl SharedWatchStore<LeaseMapRaw> {
    /// Lock the map, run `f` against the mutable map (may do async work such as
    /// DB writes), then publish the new snapshot and return it.
    ///
    /// If `f` returns an error the map is not published and the error is forwarded.
    pub(crate) async fn update<F, E, R>(&self, f: F) -> Result<R, E>
    where
        F: AsyncFnOnce(&mut LeaseMapRaw) -> Result<R, E>,
    {
        let mut guard = self.inner.lock().await;
        let mut snapshot = guard.clone();
        let result = f(&mut snapshot).await?;
        guard.clone_from(&snapshot);
        drop(guard);
        let snapshot = Arc::new(snapshot);
        // Ignore send error: it means all receivers were dropped (e.g. during shutdown).
        drop(self.tx.send(snapshot));
        Ok(result)
    }

    /// Return the current lease set for `host`, defaulting to an empty set.
    pub(crate) fn get_host(&self, host: &str) -> LeaseSources {
        self.tx.borrow().get(host).cloned().unwrap_or_default()
    }

    /// Return `true` if `host` currently has at least one active lease.
    pub(crate) fn host_has_leases(&self, host: &str) -> bool {
        self.tx.borrow().get(host).is_some_and(|s| !s.is_empty())
    }
}

pub(crate) fn lookup_host(state: &AppState, host: &str) -> Option<Host> {
    let cfg_snapshot = state.config_rx.borrow().clone();
    cfg_snapshot.hosts.get(host).cloned()
}

/// Lookup a host's config from the runtime config and apply any runtime IP/port
/// overrides stored in `AppState`. Returns `None` if the host is not present
/// in the configuration.
pub(crate) async fn lookup_host_with_overrides(
    state: &AppState,
    host: &str,
) -> Option<ResolvedHost> {
    let mut host_cfg = lookup_host(state, host)?;

    let overrides = state.host_overrides.read().await;
    if let Some(o) = overrides.get(host) {
        host_cfg.ip = o.ip.clone();
        host_cfg.port = o.port;
    }

    Some(ResolvedHost(HostWithName {
        name: host.to_string(),
        host: host_cfg,
    }))
}

/// Represents a source that holds a lease on a host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LeaseSource {
    /// Lease held by the web interface
    WebInterface,
    /// Lease held by a specific client
    Client(String),
}

/// Interval between `WoL` re-sends during a wake transition.
#[cfg(not(any(coverage, test)))]
const WOL_RESEND_INTERVAL: Duration = Duration::from_millis(500);

/// Errors returned by high-level host control operations.
#[derive(Debug, ThisError)]
pub(crate) enum HostControlError {
    #[error("No configuration found for host {0}")]
    NotFound(String),
    #[error(transparent)]
    Timeout(Report),
    #[error("Operation failed")]
    OperationFailed {
        target: HostState,
        #[source]
        report: Report,
    },
}

enum OperationOrNoop {
    Executed,
    Noop,
}

/// High-level application entrypoint for handling host state transitions.
/// Called with the already-claimed transition state (Waking or `ShuttingDown`)
/// having been atomically set before this function is invoked. Because
/// `try_begin_transition` already serialises concurrent calls, there is no
/// need to re-check the current status; we just act on the lease set.
#[tracing::instrument(skip(state), err(Debug))]
async fn handle_host_state(
    host: &str,
    state: &AppState,
    lease_set: &LeaseSources,
) -> Result<OperationOrNoop, HostControlError> {
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Handling host '{}': should_be_running={}, active_leases={:?}",
        host, should_be_running, lease_set
    );

    // Lookup host config and runtime overrides using shared helper.
    let Some(host_with_name) = lookup_host_with_overrides(state, host).await else {
        return Err(HostControlError::NotFound(host.to_string()));
    };

    // try_begin_transition already set the Waking/ShuttingDown marker and
    // ensures at most one control task runs at a time, so we unconditionally
    // perform the requested action.
    if should_be_running {
        wake_host_and_wait(&host_with_name, &state.hoststatus, &state.runtime).await
    } else {
        shutdown_host_and_wait(&host_with_name, &state.hoststatus, &state.runtime).await
    }
}

/// Attempt to spawn a host state transition task.
///
/// Determines the desired direction from the current lease set, then atomically
/// claims the transition via [`HostStatusStore::try_begin_transition`]. If a
/// control task is already in-flight for this host the call is a no-op (the
/// existing task will re-check lease state on completion and re-trigger if needed).
pub(crate) fn spawn_handle_host_state(host: &str, state: &AppState) {
    let transition_state = if state.leases.host_has_leases(host) {
        HostState::Waking
    } else {
        HostState::ShuttingDown
    };
    let operation_kind = if matches!(transition_state, HostState::Waking) {
        OperationKind::Startup
    } else {
        OperationKind::Shutdown
    };

    let host = host.to_string();
    let state = state.clone();

    tokio::spawn(
        async move {
            // Atomically claim the transition slot. Returns false if already transitioning.
            if !state
                .hoststatus
                .try_begin_transition(&host, transition_state)
                .await
            {
                debug!(host = %host, "Transition already in-flight, skipping");
                return;
            }
            // Re-read current lease state now that we've claimed the slot.
            let lease_set = state.leases.get_host(&host);
            let result = handle_host_state(&host, &state, &lease_set)
                .in_current_span()
                .await;

            // Update the per-host operation failure record.
            match result {
                Ok(_) => {
                    state.operation_failures.clear(&host).await;
                }
                Err(HostControlError::Timeout(_) | HostControlError::OperationFailed { .. }) => {
                    state.operation_failures.set(&host, OperationFailure { operation: operation_kind }).await;

                    // Fire push notifications for subscribers.
                    if let (Some(pool), Some(vapid_key)) =
                        (state.db_pool.clone(), state.vapid_key.clone())
                    {
                        let host_clone = host.clone();
                        let body = match operation_kind {
                            OperationKind::Shutdown => format!("{host} failed to shut down"),
                            OperationKind::Startup => format!("{host} failed to start up"),
                        };
                        tokio::spawn(async move {
                            match db::get_subscriptions_for_host_operation_failed(&pool, &host_clone).await {
                                Ok(subs) if !subs.is_empty() => {
                                    let payload = serde_json::json!({
                                        "title": "ShutHost",
                                        "body": body,
                                        "data": { "hostname": host_clone },
                                    })
                                    .to_string();
                                    push::send_push_notifications(&vapid_key, &pool, &subs, &payload).await;
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!(host = %host_clone, "Failed to fetch operation-failed push subscriptions: {e:#}");
                                }
                            }
                        });
                    }
                }
                Err(HostControlError::NotFound(_)) => {
                    // Config issue, not a runtime failure — leave existing failure state unchanged.
                }
            }

            if let Err(ref e) = result {
                debug!(host = %host, error = ?e, "Host state transition failed");
            }
            // Only re-check on success. If the transition failed or was a no-op,
            // immediately spawning another transition would create a tight retry loop.
            // On successful completion, re-check whether the actual state matches the
            // desired state to handle the race where the lease changed while we were
            // transitioning.
            if result.is_ok() && matches!(result, Ok(OperationOrNoop::Executed)) {
                let desired_running = state.leases.host_has_leases(&host);
                let current = state.hoststatus.get_current_state(&host);
                let is_running = matches!(current, HostState::Online);
                if desired_running != is_running {
                    spawn_handle_host_state(&host, &state);
                }
            }
        }
        .in_current_span(),
    );
}

/// Send a shutdown message to the host described by `host_with_name` and return the textual response.
async fn send_shutdown_to_address(host_with_name: &ResolvedHost) -> Result<String, Report> {
    let ip = &host_with_name.host.ip;
    let port = host_with_name.host.port;
    let secret = host_with_name.host.shared_secret.as_ref();
    let addr = format!("{ip}:{port}");
    debug!(%addr, "Connecting to host for shutdown");

    let deadline = Instant::now() + Duration::from_secs(6);

    // Connect
    let conn = timeout_at(deadline, TcpStream::connect(&addr)).await;
    let mut stream = match conn {
        Ok(Ok(s)) => s,
        Ok(e @ Err(_)) => e.wrap_err(format!("TCP connect error for {addr}"))?,
        Err(elapsed) => Err(elapsed).wrap_err(format!("Connection to {addr} timed out"))?,
    };

    let signed_message = shuthost_common::create_signed_message(
        &shuthost_common::CoordinatorMessage::Shutdown.to_string(),
        secret,
    );

    // Write
    match timeout_at(deadline, stream.write_all(signed_message.as_bytes())).await {
        Ok(Ok(())) => {}
        Ok(e @ Err(_)) => e.wrap_err("Failed to write request to stream")?,
        Err(elapsed) => Err(elapsed).wrap_err("Timeout writing request to stream")?,
    }

    // Read
    let mut buf = vec![0u8; 1024];
    let n = match timeout_at(deadline, stream.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(e @ Err(_)) => e.wrap_err("Failed to read response from stream")?,
        Err(elapsed) => Err(elapsed).wrap_err("Timeout reading response from stream")?,
    };

    let Some(data) = buf.get(..n) else {
        unreachable!("Read data size should always be valid, as its <= buffer size");
    };

    Ok(String::from_utf8_lossy(data).to_string())
}

/// Send `WoL` packets and poll until the host comes online, re-sending the `WoL`
/// magic packet every [`WOL_RESEND_INTERVAL`] until the deadline to account for
/// UDP packet loss during boot. The re-send task is aborted as soon as the host
/// is confirmed online or the deadline is reached.
///
/// On any error (including timeout), the host status is reset to `Offline` since
/// the wake did not succeed.
async fn wake_host_and_wait(
    host_with_name: &ResolvedHost,
    hoststatus: &HostStatusStore,
    runtime: &RuntimeConfig,
) -> Result<OperationOrNoop, HostControlError> {
    if host_with_name.host.mac.eq_ignore_ascii_case("disablewol") {
        info!(host = %host_with_name.name, "WOL disabled for host");
        return Ok(OperationOrNoop::Noop);
    }

    let wake_secs = host_with_name
        .host
        .wake_timeout_secs
        .unwrap_or(runtime.default_wake_timeout_secs);
    let deadline = Instant::now() + Duration::from_secs(wake_secs);

    info!(host = %host_with_name.name, mac = %host_with_name.host.mac, "Sending WoL packet");

    #[cfg(not(any(coverage, test)))]
    if let Err(e) = wol::send_magic_packet(&host_with_name.host.mac, "255.255.255.255") {
        hoststatus
            .force_set(&host_with_name.name, HostState::Offline)
            .await;
        return Err(HostControlError::OperationFailed {
            target: HostState::Online,
            report: e.wrap_err("Failed to send WoL packet"),
        });
    }

    // Re-send WoL every WOL_RESEND_INTERVAL in a background task until we know the host
    // is online. Aborted when the poll future returns (success or timeout).
    #[cfg(not(any(coverage, test)))]
    let wol_resend_handle = {
        let mac = host_with_name.host.mac.clone();
        tokio::spawn(async move {
            let mut ticker = interval(WOL_RESEND_INTERVAL);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            ticker.tick().await; // skip the immediate tick; first re-send is after one interval
            loop {
                ticker.tick().await;
                if let Err(e) = wol::send_magic_packet(&mac, "255.255.255.255") {
                    debug!("WoL re-send failed: {e}");
                }
            }
        })
    };

    let poll_result = poll_until_host_state(
        host_with_name,
        HostState::Online,
        deadline,
        runtime.transition_poll_interval_ms,
        hoststatus,
    )
    .await;

    #[cfg(not(any(coverage, test)))]
    wol_resend_handle.abort();

    match poll_result {
        Ok(()) => Ok(OperationOrNoop::Executed),
        Err(e) => {
            hoststatus
                .force_set(&host_with_name.name, HostState::Offline)
                .await;
            match e {
                PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
            }
        }
    }
}

/// Send shutdown command to host and wait until offline.
///
/// On any error (including timeout), the host status is reset to `Online` since
/// the shutdown did not succeed.
async fn shutdown_host_and_wait(
    host_with_name: &ResolvedHost,
    hoststatus: &HostStatusStore,
    runtime: &RuntimeConfig,
) -> Result<OperationOrNoop, HostControlError> {
    // Send shutdown to the address
    let resp = match send_shutdown_to_address(host_with_name).await {
        Ok(r) => r,
        Err(e) => {
            hoststatus
                .force_set(&host_with_name.name, HostState::Online)
                .await;
            return Err(HostControlError::OperationFailed {
                target: HostState::Offline,
                report: e,
            });
        }
    };

    if resp.contains("ERROR") {
        hoststatus
            .force_set(&host_with_name.name, HostState::Online)
            .await;
        return Err(HostControlError::OperationFailed {
            target: HostState::Offline,
            report: eyre::eyre!("Agent rejected shutdown command: {resp}"),
        });
    }

    let shutdown_secs = host_with_name
        .host
        .shutdown_timeout_secs
        .unwrap_or(runtime.default_shutdown_timeout_secs);
    let deadline = Instant::now() + Duration::from_secs(shutdown_secs);
    match poll_until_host_state(
        host_with_name,
        HostState::Offline,
        deadline,
        runtime.transition_poll_interval_ms,
        hoststatus,
    )
    .await
    {
        Ok(()) => Ok(OperationOrNoop::Executed),
        Err(e) => {
            hoststatus
                .force_set(&host_with_name.name, HostState::Online)
                .await;
            match e {
                PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
            }
        }
    }
}

/// Poll for the desired host state and handle errors uniformly.
/// Used by the M2M API sync path to wait for a host to reach the desired state.
pub(crate) async fn poll_and_wait(
    host_with_name: &ResolvedHost,
    hoststatus: &HostStatusStore,
    desired_state: HostState,
    deadline: Instant,
    runtime: &RuntimeConfig,
) -> Result<(), HostControlError> {
    match poll_until_host_state(
        host_with_name,
        desired_state,
        deadline,
        runtime.transition_poll_interval_ms,
        hoststatus,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(e) => match e {
            PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
        },
    }
}
