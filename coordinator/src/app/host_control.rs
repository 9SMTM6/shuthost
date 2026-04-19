//! Host control application logic (non-HTTP). This module contains the core
//! operations for waking/shutting hosts and polling their state.

use alloc::sync::Arc;
use core::{ops, time::Duration};
use std::collections::{HashMap, HashSet};

use eyre::{Context as _, Report};
use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    sync::{Mutex, watch},
    time::{Instant, timeout_at},
};
#[cfg(not(any(coverage, test)))]
use tokio::time::{MissedTickBehavior, interval};
use tracing::{Instrument as _, debug, info};

use crate::app::{AppState, runtime::PollError, runtime::poll_until_host_state, state::HostState};

#[cfg(not(any(coverage, test)))]
use crate::wol;
use crate::{app::state::HostStatusTx, config::Host};

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

/// Watch channel sender for the lease map.
pub(crate) type LeaseTx = watch::Sender<Arc<LeaseMapRaw>>;
/// Watch channel receiver for the lease map.
pub(crate) type LeaseRx = watch::Receiver<Arc<LeaseMapRaw>>;

/// Serialized lease map state: writes are serialized via a [`Mutex`], and all
/// mutations are published to a [`watch`] channel so background tasks can
/// subscribe to changes.
pub(crate) struct LeaseState {
    inner: Mutex<LeaseMapRaw>,
    tx: LeaseTx,
}

impl LeaseState {
    /// Create a new `LeaseState` from an initial map.
    /// Returns an `Arc<LeaseState>` and an initial [`LeaseRx`] receiver.
    pub(crate) fn new(initial: LeaseMapRaw) -> (Arc<Self>, LeaseRx) {
        let (tx, rx) = watch::channel(Arc::new(initial.clone()));
        (
            Arc::new(Self {
                inner: Mutex::new(initial),
                tx,
            }),
            rx,
        )
    }

    /// Lock the map, run `f` against the mutable map (may do async work such as
    /// DB writes), then publish the new snapshot and return it.
    ///
    /// If `f` returns an error the map is not published and the error is forwarded.
    pub(crate) async fn update<F, E>(&self, f: F) -> Result<(), E>
    where
        F: AsyncFnOnce(&mut LeaseMapRaw) -> Result<(), E>,
    {
        let mut guard = self.inner.lock().await;
        let mut snapshot = guard.clone();
        f(&mut snapshot).await?;
        guard.clone_from(&snapshot);
        drop(guard);
        let snapshot = Arc::new(snapshot);
        // Ignore send error: it means all receivers were dropped (e.g. during shutdown).
        drop(self.tx.send(snapshot));
        Ok(())
    }

    /// Read the current snapshot cheaply without acquiring the write mutex.
    pub(crate) fn borrow(&self) -> watch::Ref<'_, Arc<LeaseMapRaw>> {
        self.tx.borrow()
    }

    /// Subscribe to future changes of the lease map.
    pub(crate) fn subscribe(&self) -> LeaseRx {
        self.tx.subscribe()
    }
}

/// Lookup a host's config from the runtime config and apply any runtime IP/port
/// overrides stored in `AppState`. Returns `None` if the host is not present
/// in the configuration.
pub(crate) async fn lookup_host_with_overrides(
    state: &AppState,
    host: &str,
) -> Option<ResolvedHost> {
    let cfg_snapshot = state.config_rx.borrow().clone();
    let mut host_cfg = match cfg_snapshot.hosts.get(host) {
        Some(h) => h.clone(),
        None => return None,
    };

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

const DEFAULT_POLL_INTERVAL_MS: u64 = 200;

/// Default wake timeout: how long to wait for a host to come online after sending WoL packets.
/// Can be overridden per host via `wake_timeout_secs` in the config.
pub(crate) const DEFAULT_WAKE_TIMEOUT_SECS: u64 = 120;

/// Default shutdown timeout: how long to wait for a host to go offline after sending a shutdown command.
/// Can be overridden per host via `shutdown_timeout_secs` in the config.
pub(crate) const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 20;

/// Interval between WoL re-sends during a wake transition.
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
    OperationFailed(HostState, #[source] Report),
}

/// High-level application entrypoint for handling host state transitions.
/// Returns a `HostControlError` describing any failure.
#[tracing::instrument(skip(state), err(Debug))]
async fn handle_host_state(
    host: &str,
    state: &AppState,
    lease_set: &LeaseSources,
) -> Result<(), HostControlError> {
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Checking state for host '{}': should_be_running={}, active_leases={:?}",
        host, should_be_running, lease_set
    );

    let current_state = {
        let hoststatus_rx = state.hoststatus_rx.borrow();
        hoststatus_rx
            .get(host)
            .copied()
            .unwrap_or(HostState::Offline)
    };

    debug!("Current state for host '{}': {:?}", host, current_state);

    // Lookup host config and runtime overrides using shared helper.
    let Some(host_with_name) = lookup_host_with_overrides(state, host).await else {
        return Err(HostControlError::NotFound(host.to_string()));
    };

    if should_be_running && current_state == HostState::Offline {
        wake_host_and_wait(&host_with_name, &state.hoststatus_tx).await
    } else if !should_be_running && current_state == HostState::Online {
        shutdown_host_and_wait(&host_with_name, &state.hoststatus_tx).await
    } else {
        Ok(())
    }
}

/// Helper function to spawn an async task that handles host state changes.
/// Acquires a per-host serialization lock before acting, so at most one
/// wake/shutdown task runs at a time for a given host. The task re-reads the
/// current lease state under the lock, ensuring it acts on the most recent
/// desired state rather than whatever was current at call time.
pub(crate) fn spawn_handle_host_state(host: &str, state: &AppState) {
    let host = host.to_string();
    let state = state.clone();

    // Get-or-create the per-host transition lock (outer std::Mutex held only briefly).
    let lock = {
        let mut locks = state
            .host_transition_locks
            .lock()
            .expect("host_transition_locks poisoned");
        Arc::clone(locks.entry(host.clone()).or_insert_with(|| Arc::new(Mutex::new(()))))
    };

    tokio::spawn(
        async move {
            // Serialize: queue behind any in-flight wake or shutdown for this host.
            let _guard = lock.lock().await;
            // Re-derive current desired state once we hold the lock, so we act on
            // the most recent lease snapshot rather than a potentially stale one.
            let lease_set = state.leases.borrow().get(&host).cloned().unwrap_or_default();
            drop(
                handle_host_state(&host, &state, &lease_set)
                    .in_current_span()
                    .await,
            );
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

/// Send WoL packets and poll until the host comes online, re-sending the WoL
/// magic packet every [`WOL_RESEND_INTERVAL`] until the deadline to account for
/// UDP packet loss during boot. The re-send task is aborted as soon as the host
/// is confirmed online or the deadline is reached.
async fn wake_host_and_wait(
    host_with_name: &ResolvedHost,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), HostControlError> {
    if host_with_name.host.mac.eq_ignore_ascii_case("disablewol") {
        info!(host = %host_with_name.name, "WOL disabled for host");
        return Ok(());
    }

    let wake_secs = host_with_name
        .host
        .wake_timeout_secs
        .unwrap_or(DEFAULT_WAKE_TIMEOUT_SECS);
    let deadline = Instant::now() + Duration::from_secs(wake_secs);

    info!(host = %host_with_name.name, mac = %host_with_name.host.mac, "Sending WoL packet");

    #[cfg(not(any(coverage, test)))]
    if let Err(e) = wol::send_magic_packet(&host_with_name.host.mac, "255.255.255.255") {
        return Err(HostControlError::OperationFailed(
            HostState::Online,
            e.wrap_err("Failed to send WoL packet"),
        ));
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
        DEFAULT_POLL_INTERVAL_MS,
        hoststatus_tx,
    )
    .await;

    #[cfg(not(any(coverage, test)))]
    wol_resend_handle.abort();

    match poll_result {
        Ok(()) => Ok(()),
        Err(e) => match e {
            PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
            PollError::CoordinatorShuttingDown => {
                Err(HostControlError::OperationFailed(HostState::Online, e.into()))
            }
        },
    }
}

/// Send shutdown command to host and wait until offline.
async fn shutdown_host_and_wait(
    host_with_name: &ResolvedHost,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), HostControlError> {
    // Send shutdown to the address
    let resp = match send_shutdown_to_address(host_with_name).await {
        Ok(r) => r,
        Err(e) => return Err(HostControlError::OperationFailed(HostState::Offline, e)),
    };

    if resp.contains("ERROR") {
        return Err(HostControlError::OperationFailed(
            HostState::Offline,
            eyre::eyre!("Agent rejected shutdown command: {resp}"),
        ));
    }

    let shutdown_secs = host_with_name
        .host
        .shutdown_timeout_secs
        .unwrap_or(DEFAULT_SHUTDOWN_TIMEOUT_SECS);
    let deadline = Instant::now() + Duration::from_secs(shutdown_secs);
    match poll_until_host_state(
        host_with_name,
        HostState::Offline,
        deadline,
        DEFAULT_POLL_INTERVAL_MS,
        hoststatus_tx,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(e) => match e {
            PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
            PollError::CoordinatorShuttingDown => {
                Err(HostControlError::OperationFailed(HostState::Offline, e.into()))
            }
        },
    }
}

/// Poll for the desired host state and handle errors uniformly.
/// Used by the M2M API sync path to wait for a host to reach the desired state.
pub(crate) async fn poll_and_wait(
    host_with_name: &ResolvedHost,
    hoststatus_tx: &HostStatusTx,
    desired_state: HostState,
    deadline: Instant,
) -> Result<(), HostControlError> {
    match poll_until_host_state(
        host_with_name,
        desired_state,
        deadline,
        DEFAULT_POLL_INTERVAL_MS,
        hoststatus_tx,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(e) => match e {
            PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
            PollError::CoordinatorShuttingDown => {
                Err(HostControlError::OperationFailed(desired_state, e.into()))
            }
        },
    }
}
