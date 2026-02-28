//! Host control application logic (non-HTTP). This module contains the core
//! operations for waking/shutting hosts and polling their state.

use alloc::sync::Arc;
use core::time::Duration;
use std::collections::{HashMap, HashSet};

use eyre::Context as _;
use eyre::Report;
use futures::future::BoxFuture;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error as ThisError;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, watch};
use tokio::time::timeout;
use tracing::Instrument as _;
use tracing::{debug, info};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::app::runtime::PollError;
use crate::app::{AppState, runtime::poll_until_host_state, state::HostState};

#[cfg(not(any(coverage, test)))]
use crate::wol;
use crate::{app::state::HostStatusTx, config::Host};

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

    // TODO: Consider the need for these abstractions, BoxFuture, higher order lifetimes etc.
    /// Lock the map, run `f` against the mutable map (may do async work such as
    /// DB writes), then publish the new snapshot and return it.
    ///
    /// If `f` returns an error the map is not published and the error is forwarded.
    pub(crate) async fn update<F, E>(&self, f: F) -> Result<Arc<LeaseMapRaw>, E>
    where
        F: for<'future_life> FnOnce(
            &'future_life mut LeaseMapRaw,
        ) -> BoxFuture<'future_life, Result<(), E>>,
    {
        let mut guard = self.inner.lock().await;
        f(&mut guard).await?;
        let snapshot = Arc::new(guard.clone());
        drop(guard);
        // Ignore send error: it means all receivers were dropped (e.g. during shutdown).
        drop(self.tx.send(snapshot.clone()));
        Ok(snapshot)
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

/// Represents a source that holds a lease on a host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LeaseSource {
    /// Lease held by the web interface
    WebInterface,
    /// Lease held by a specific client
    Client(String),
}

/// Poll timeout used by wrappers that wait for a host to reach a desired state.
const DEFAULT_POLL_TIMEOUT_SECS: u64 = 60;
const DEFAULT_POLL_INTERVAL_MS: u64 = 200;

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
#[tracing::instrument(skip_all, err(Debug))]
pub(crate) async fn handle_host_state(
    host: &str,
    &AppState {
        ref config_rx,
        ref hoststatus_rx,
        ref hoststatus_tx,
        ..
    }: &AppState,
    lease_set: &LeaseSources,
) -> Result<(), HostControlError> {
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Checking state for host '{}': should_be_running={}, active_leases={:?}",
        host, should_be_running, lease_set
    );

    let current_state = {
        let hoststatus_rx = hoststatus_rx.borrow();
        hoststatus_rx
            .get(host)
            .copied()
            .unwrap_or(HostState::Offline)
    };

    debug!("Current state for host '{}': {:?}", host, current_state);

    // Lookup host config
    let cfg_snapshot = config_rx.borrow().clone();
    let host_cfg = match cfg_snapshot.hosts.get(host) {
        Some(h) => h.clone(),
        None => return Err(HostControlError::NotFound(host.to_string())),
    };

    if should_be_running && current_state == HostState::Offline {
        wake_host_and_wait(host, &host_cfg, hoststatus_tx).await
    } else if !should_be_running && current_state == HostState::Online {
        shutdown_host_and_wait(host, &host_cfg, hoststatus_tx).await
    } else {
        Ok(())
    }
}

/// Helper function to spawn an async task that handles host state changes.
/// This clones the necessary state fields and spawns a background task to handle the host state change.
#[tracing::instrument(skip(state))]
pub(crate) fn spawn_handle_host_state(host: &str, lease_set: &LeaseSources, state: &AppState) {
    let host = host.to_string();
    let lease_set = lease_set.clone();
    let state = state.clone();

    tokio::spawn(
        async move {
            drop(
                handle_host_state(&host, &state, &lease_set)
                    .in_current_span()
                    .await,
            );
        }
        .in_current_span(),
    );
}

/// Send a shutdown message to a host address and return the textual response.
async fn send_shutdown_to_address(
    ip: &str,
    port: u16,
    secret: &secrecy::SecretString,
) -> Result<String, Report> {
    let addr = format!("{ip}:{port}");
    debug!(%addr, "Connecting to host for shutdown");

    // Connect with timeout
    let conn = timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await;
    let mut stream = match conn {
        Ok(Ok(s)) => s,
        Ok(e @ Err(_)) => e.wrap_err(format!("TCP connect error for {addr}"))?,
        Err(elapsed) => Err(elapsed).wrap_err(format!("Connection to {addr} timed out"))?,
    };

    let signed_message = shuthost_common::create_signed_message(
        &shuthost_common::CoordinatorMessage::Shutdown.to_string(),
        secret,
    );

    // Write with timeout
    match timeout(
        Duration::from_secs(2),
        stream.write_all(signed_message.as_bytes()),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(e @ Err(_)) => e.wrap_err("Failed to write request to stream")?,
        Err(elapsed) => Err(elapsed).wrap_err("Timeout writing request to stream")?,
    }

    // Read with timeout
    let mut buf = vec![0u8; 1024];
    let n = match timeout(Duration::from_secs(2), stream.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(e @ Err(_)) => e.wrap_err("Failed to read response from stream")?,
        Err(elapsed) => Err(elapsed).wrap_err("Timeout reading response from stream")?,
    };

    let Some(data) = buf.get(..n) else {
        unreachable!("Read data size should always be valid, as its <= buffer size");
    };

    Ok(String::from_utf8_lossy(data).to_string())
}

/// Send a `WoL` packet (via crate-level `wol` helper) and then wait until the
/// host becomes online by polling runtime state.
async fn wake_host_and_wait(
    host_name: &str,
    host_cfg: &Host,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), HostControlError> {
    if host_cfg.mac.eq_ignore_ascii_case("disablewol") {
        info!(host = %host_name, "WOL disabled for host");
        return Ok(());
    }
    info!(host = %host_name, mac = %host_cfg.mac, "Sending WoL packet");

    // send_magic_packet is behind cfg flags in some builds; call the wrapper
    #[cfg(not(any(coverage, test)))]
    if let Err(e) = wol::send_magic_packet(&host_cfg.mac, "255.255.255.255") {
        return Err(HostControlError::OperationFailed(
            HostState::Online,
            e.wrap_err("Failed to send WoL packet"),
        ));
    }

    poll_and_wait(host_name, host_cfg, hoststatus_tx, HostState::Online).await
}

/// Send shutdown command to host and wait until offline.
async fn shutdown_host_and_wait(
    host_name: &str,
    host_cfg: &Host,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), HostControlError> {
    // Send shutdown to the address
    let _resp = match send_shutdown_to_address(
        &host_cfg.ip,
        host_cfg.port,
        host_cfg.shared_secret.as_ref(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return Err(HostControlError::OperationFailed(HostState::Offline, e)),
    };

    poll_and_wait(host_name, host_cfg, hoststatus_tx, HostState::Offline).await
}

/// Poll for the desired host state and handle errors uniformly.
async fn poll_and_wait(
    host_name: &str,
    host_cfg: &Host,
    hoststatus_tx: &HostStatusTx,
    desired_state: HostState,
) -> Result<(), HostControlError> {
    match poll_until_host_state(
        host_name,
        host_cfg,
        desired_state,
        DEFAULT_POLL_TIMEOUT_SECS,
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
