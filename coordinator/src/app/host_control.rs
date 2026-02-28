//! Host control application logic (non-HTTP). This module contains the core
//! operations for waking/shutting hosts and polling their state. HTTP-specific
//! response mapping lives in `http/m2m/host_control.rs`.

use core::time::Duration;

use eyre::Report;
use eyre::eyre;
use thiserror::Error as ThisError;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::Instrument as _;
use tracing::{debug, info};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::app::{AppState, runtime::poll_until_host_state, state::HostState};

#[cfg(not(any(coverage, test)))]
use crate::wol;
use crate::{
    app::{
        runtime,
        state::{ConfigRx, HostStatusRx, HostStatusTx},
    },
    config::Host,
    websocket::LeaseSources,
};

/// Poll timeout used by wrappers that wait for a host to reach a desired state.
const DEFAULT_POLL_TIMEOUT_SECS: u64 = 60;
const DEFAULT_POLL_INTERVAL_MS: u64 = 200;

/// Errors returned by high-level host control operations.
#[derive(Debug, ThisError)]
pub(crate) enum HostControlError {
    #[error("Unknown host")]
    NotFound,
    #[error("Timeout")]
    Timeout(#[source] Report),
    #[error("Operation failed")]
    OperationFailed(HostState, #[source] Report),
}

/// High-level application entrypoint for handling host state transitions.
/// Returns a `HostControlError` describing any failure.
#[tracing::instrument(skip_all, err(Debug))]
pub(crate) async fn handle_host_state(
    host: &str,
    lease_set: &LeaseSources,
    hoststatus_rx: &HostStatusRx,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
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

    let hoststatus_tx = hoststatus_tx.clone();

    debug!("Current state for host '{}': {:?}", host, current_state);

    // Lookup host config
    let cfg_snapshot = config_rx.borrow().clone();
    let host_cfg = match cfg_snapshot.hosts.get(host) {
        Some(h) => h.clone(),
        None => return Err(HostControlError::NotFound),
    };

    if should_be_running && current_state == HostState::Offline {
        wake_host_and_wait(host, &host_cfg, config_rx, &hoststatus_tx).await
    } else if !should_be_running && current_state == HostState::Online {
        shutdown_host_and_wait(host, &host_cfg, config_rx, &hoststatus_tx).await
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
    let hoststatus_rx = state.hoststatus_rx.clone();
    let config_rx = state.config_rx.clone();
    let hoststatus_tx = state.hoststatus_tx.clone();

    tokio::spawn(
        async move {
            drop(
                handle_host_state(
                    &host,
                    &lease_set,
                    &hoststatus_rx,
                    &config_rx,
                    &hoststatus_tx,
                )
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
        Ok(Err(e)) => return Err(eyre!(format!("TCP connect error to {addr}: {}", e))),
        Err(_) => return Err(eyre!(format!("Connection to {addr} timed out"))),
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
        Ok(Err(e)) => return Err(eyre!(format!("Failed to write request to stream: {}", e))),
        Err(_) => return Err(eyre!("Timeout writing request to stream")),
    }

    // Read with timeout
    let mut buf = vec![0u8; 1024];
    let n = match timeout(Duration::from_secs(2), stream.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(eyre!(format!("Failed to read response from stream: {}", e))),
        Err(_) => return Err(eyre!("Timeout reading response from stream")),
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
    config_rx: &ConfigRx,
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
            eyre!(format!("Failed to send WoL packet: {}", e)),
        ));
    }

    poll_and_wait(host_name, config_rx, hoststatus_tx, HostState::Online).await
}

/// Send shutdown command to host and wait until offline.
async fn shutdown_host_and_wait(
    host_name: &str,
    host_cfg: &Host,
    config_rx: &ConfigRx,
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

    poll_and_wait(host_name, config_rx, hoststatus_tx, HostState::Offline).await
}

/// Poll for the desired host state and handle errors uniformly.
async fn poll_and_wait(
    host_name: &str,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
    desired_state: HostState,
) -> Result<(), HostControlError> {
    match poll_until_host_state(
        host_name,
        desired_state,
        DEFAULT_POLL_TIMEOUT_SECS,
        DEFAULT_POLL_INTERVAL_MS,
        config_rx,
        hoststatus_tx,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(e) => match e {
            runtime::PollError::Timeout { .. } => Err(HostControlError::Timeout(e.into())),
            runtime::PollError::NotFound => Err(HostControlError::NotFound),
            runtime::PollError::CoordinatorShuttingDown => {
                Err(HostControlError::OperationFailed(desired_state, e.into()))
            }
        },
    }
}
