//! Host control logic for wake-on-LAN and shutdown operations.

use core::time::Duration;

use axum::http::StatusCode;
use eyre::{Result, WrapErr as _};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::timeout,
};
use tracing::{debug, error, info, warn};

use shuthost_common::{CoordinatorMessage, create_signed_message};

#[cfg(not(any(coverage, test)))]
use crate::wol::send_magic_packet;
use crate::{
    config::Host,
    http::{
        polling::poll_until_host_state,
        server::{AppState, ConfigRx, HostStatusRx, HostStatusTx},
    },
    websocket::LeaseSources,
};

/// Timeout for TCP operations
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Handle host state changes based on lease status.
///
/// # Errors
///
/// Returns an error if:
/// - Waking up the host fails (e.g., network issues, invalid configuration)
/// - Shutting down the host fails (e.g., network issues, invalid configuration)
/// - The host fails to reach the desired state within the timeout period (60 seconds)
pub(crate) async fn handle_host_state(
    host: &str,
    lease_set: &LeaseSources,
    hoststatus_rx: &HostStatusRx,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), (StatusCode, String)> {
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Checking state for host '{}': should_be_running={}, active_leases={:?}",
        host, should_be_running, lease_set
    );

    let host_is_on = {
        let hoststatus_rx = hoststatus_rx.borrow();
        hoststatus_rx.get(host).copied().unwrap_or(false)
    };

    debug!("Current state for host '{}': is_on={}", host, host_is_on);

    if should_be_running && !host_is_on {
        wake_up_host(host, lease_set, config_rx, hoststatus_tx).await?;
    } else if !should_be_running && host_is_on {
        shutdown_host_action(host, config_rx, hoststatus_tx).await?;
    } else {
        debug!(
            "No action needed for host '{}' (is_on={}, should_be_running={})",
            host, host_is_on, should_be_running
        );
    }

    Ok(())
}

/// Wake up a host if WOL is enabled, and poll until online.
async fn wake_up_host(
    host: &str,
    lease_set: &LeaseSources,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), (StatusCode, String)> {
    info!(
        "Host '{host}' needs to wake up - has {} active lease(s): {:?}",
        lease_set.len(),
        lease_set
    );
    let host_config = get_host_config(host, config_rx).map_err(|(sc, err)| (sc, err.to_owned()))?;
    if host_config.mac.eq_ignore_ascii_case("disablewol") {
        info!("WOL disabled for host '{}' (MAC set to 'disableWOL')", host);
        Ok(())
    } else {
        info!(
            "Sending WoL packet to '{}' (MAC: {})",
            host, host_config.mac
        );
        #[cfg(not(any(coverage, test)))]
        send_magic_packet(&host_config.mac, "255.255.255.255").map_err(|e| {
            error!("Failed to send WoL packet to '{}': {}", host, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to send wake packet".to_owned(),
            )
        })?;
        info!("Successfully sent WoL packet to '{}'", host);
        // Poll until host is online, updating global state
        poll_until_host_state_wrapped(host, true, config_rx, hoststatus_tx).await
    }
}

/// Shut down a host and poll until offline.
async fn shutdown_host_action(
    host: &str,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), (StatusCode, String)> {
    info!("Host '{host}' should shut down - no active leases");
    send_shutdown_to_host(host, config_rx)
        .await
        .map_err(|(sc, err)| (sc, err.to_owned()))?;
    // Poll until host is offline, updating global state
    poll_until_host_state_wrapped(host, false, config_rx, hoststatus_tx).await
}

/// Helper function to spawn an async task that handles host state changes.
/// This clones the necessary state fields and spawns a background task to handle the host state change.
pub(crate) fn spawn_handle_host_state(host: &str, lease_set: &LeaseSources, state: &AppState) {
    let host = host.to_string();
    let lease_set = lease_set.clone();
    let hoststatus_rx = state.hoststatus_rx.clone();
    let config_rx = state.config_rx.clone();
    let hoststatus_tx = state.hoststatus_tx.clone();

    tokio::spawn(async move {
        drop(
            handle_host_state(
                &host,
                &lease_set,
                &hoststatus_rx,
                &config_rx,
                &hoststatus_tx,
            )
            .await,
        );
    });
}

/// Poll until a host reaches the desired state, mapping polling errors to
/// a `(StatusCode::GATEWAY_TIMEOUT, String)` and logging a warning.
async fn poll_until_host_state_wrapped(
    host: &str,
    desired_state: bool,
    config_rx: &ConfigRx,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), (StatusCode, String)> {
    poll_until_host_state(host, desired_state, 60, 200, config_rx, hoststatus_tx)
        .await
        .map_err(|e| {
            warn!("{e}");
            (StatusCode::GATEWAY_TIMEOUT, e)
        })
}

/// Get host configuration from the current config.
pub(crate) fn get_host_config(
    host_name: &str,
    config_rx: &ConfigRx,
) -> Result<Host, (StatusCode, &'static str)> {
    let config = config_rx.borrow();
    match config.hosts.get(host_name) {
        Some(host) => {
            debug!(
                "Found configuration for host '{}': ip={}, mac={}",
                host_name, host.ip, host.mac
            );
            Ok(host.clone())
        }
        None => {
            warn!("No configuration found for host '{}'", host_name);
            Err((StatusCode::NOT_FOUND, "Unknown host"))
        }
    }
}

/// Execute a TCP shutdown request to a host.
async fn execute_tcp_shutdown_request(
    stream: &mut TcpStream,
    request: &[u8],
    response_buf: &mut [u8],
) -> Result<usize> {
    timeout(REQUEST_TIMEOUT, stream.writable())
        .await
        .wrap_err("Timeout waiting for stream to be writable")?
        .wrap_err("Stream not writable")?;

    debug!("Sending shutdown message...");
    timeout(REQUEST_TIMEOUT, stream.write_all(request))
        .await
        .wrap_err("Timeout writing request to stream")?
        .wrap_err("Failed to write request to stream")?;

    timeout(REQUEST_TIMEOUT, stream.read(response_buf))
        .await
        .wrap_err("Timeout reading response from stream")?
        .wrap_err("Failed to read response from stream")
}

/// Send a shutdown command to a host via TCP.
pub(crate) async fn send_shutdown_to_address(
    ip: &str,
    port: u16,
    secret: &secrecy::SecretString,
) -> Result<String> {
    let addr = format!("{ip}:{port}");
    debug!("Connecting to {}", addr);

    let mut stream = timeout(REQUEST_TIMEOUT, TcpStream::connect(&addr))
        .await
        .wrap_err(format!("Connection to {addr} timed out"))?
        .wrap_err(format!("TCP connect error to {addr}"))?;

    let signed_message = create_signed_message(&CoordinatorMessage::Shutdown.to_string(), secret);

    let mut buf = vec![0; 1024];
    let n = execute_tcp_shutdown_request(&mut stream, signed_message.as_bytes(), &mut buf).await?;

    let Some(data) = buf.get(..n) else {
        unreachable!("Read data size should always be valid, as its >= buffer size");
    };
    Ok(String::from_utf8_lossy(data).to_string())
}

/// Send a shutdown command to a host.
async fn send_shutdown_to_host(
    host: &str,
    config_rx: &ConfigRx,
) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to shutdown host '{}'", host);

    let host_config = get_host_config(host, config_rx)?;

    info!(
        "Sending shutdown command to '{}' ({}:{})",
        host, host_config.ip, host_config.port
    );
    send_shutdown_to_address(
        &host_config.ip,
        host_config.port,
        host_config.shared_secret.as_ref(),
    )
    .await
    .map_err(|e| {
        error!("Failed to send shutdown command to '{}': {}", host, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send shutdown command",
        )
    })?;

    info!("Successfully sent shutdown command to '{}'", host);
    Ok(())
}
