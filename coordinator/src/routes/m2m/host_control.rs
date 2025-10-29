//! Host control logic for wake-on-LAN and shutdown operations.

use std::time::Duration;

use axum::http::StatusCode;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
    time::timeout,
};
use tracing::{debug, error, info, warn};

use shuthost_common::create_signed_message;

use crate::{
    config::Host,
    http::{AppState, polling::poll_until_host_state},
    routes::m2m::leases::LeaseSource,
    wol::send_magic_packet,
};

/// Timeout for TCP operations
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Handle host state changes based on lease status.
pub async fn handle_host_state(
    host: &str,
    lease_set: &std::collections::HashSet<LeaseSource>,
    state: &AppState,
) -> Result<(), (StatusCode, String)> {
    let should_be_running = !lease_set.is_empty();

    debug!(
        "Checking state for host '{}': should_be_running={}, active_leases={:?}",
        host, should_be_running, lease_set
    );

    let host_is_on = {
        let hoststatus_rx = state.hoststatus_rx.borrow();
        hoststatus_rx.get(host).copied().unwrap_or(false)
    };

    debug!("Current state for host '{}': is_on={}", host, host_is_on);

    let poll_with_err = |desired_state: bool| async move {
        match poll_until_host_state(
            host,
            desired_state,
            60,
            200,
            &state.config_rx,
            &state.hoststatus_tx,
        )
        .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!("{e}");
                Err((
                    StatusCode::GATEWAY_TIMEOUT,
                    format!(
                        "Timeout waiting for host {host} to become {}",
                        if desired_state { "online" } else { "offline" }
                    ),
                ))
            }
        }
    };

    if should_be_running && !host_is_on {
        info!(
            "Host '{host}' needs to wake up - has {} active lease(s): {:?}",
            lease_set.len(),
            lease_set
        );
        wake_host(host, state).map_err(|(sc, err)| (sc, err.to_owned()))?;
        // Poll until host is online, updating global state
        poll_with_err(true).await?;
    } else if !should_be_running && host_is_on {
        info!("Host '{host}' should shut down - no active leases");
        shutdown_host(host, state)
            .await
            .map_err(|(sc, err)| (sc, err.to_owned()))?;
        // Poll until host is offline, updating global state
        poll_with_err(false).await?;
    } else {
        debug!(
            "No action needed for host '{}' (is_on={}, should_be_running={})",
            host, host_is_on, should_be_running
        );
    }

    Ok(())
}

/// Get host configuration from the current config.
pub fn get_host_config(
    host_name: &str,
    state: &AppState,
) -> Result<Host, (StatusCode, &'static str)> {
    let config = state.config_rx.borrow();
    match config.hosts.get(host_name) {
        Some(host) => {
            debug!(
                "Found configuration for host '{}': ip={}, mac={}",
                host_name, host.ip, host.mac
            );
            Ok(host.clone())
        }
        None => {
            error!("No configuration found for host '{}'", host_name);
            Err((StatusCode::NOT_FOUND, "Unknown host"))
        }
    }
}

/// Send a wake-on-LAN packet to wake up a host.
fn wake_host(host_name: &str, state: &AppState) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to wake host '{}'", host_name);

    let host_config = get_host_config(host_name, state)?;

    if host_config.mac.eq_ignore_ascii_case("disablewol") {
        info!(
            "WOL disabled for host '{}' (MAC set to 'disableWOL')",
            host_name
        );
        return Ok(());
    }

    info!(
        "Sending WoL packet to '{}' (MAC: {})",
        host_name, host_config.mac
    );
    send_magic_packet(&host_config.mac, "255.255.255.255").map_err(|e| {
        error!("Failed to send WoL packet to '{}': {}", host_name, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to send wake packet",
        )
    })?;

    info!("Successfully sent WoL packet to '{}'", host_name);
    Ok(())
}

/// Execute a TCP shutdown request to a host.
async fn execute_tcp_shutdown_request(
    stream: &mut TcpStream,
    request: &[u8],
    response_buf: &mut [u8],
) -> Result<usize, String> {
    timeout(REQUEST_TIMEOUT, stream.writable())
        .await
        .map_err(|e| format!("Stream not writable (timeout): {e}"))?
        .map_err(|e| format!("Stream not writable: {e}"))?;

    debug!("Sending shutdown message...");
    timeout(REQUEST_TIMEOUT, stream.write_all(request))
        .await
        .map_err(|e| format!("Write failed (timeout): {e}"))?
        .map_err(|e| format!("Write failed: {e}"))?;

    timeout(REQUEST_TIMEOUT, stream.read(response_buf))
        .await
        .map_err(|e| format!("Read timed out: {e}"))?
        .map_err(|e| format!("Read failed: {e}"))
}

/// Send a shutdown command to a host via TCP.
pub async fn send_shutdown(ip: &str, port: u16, message: &str) -> Result<String, String> {
    let addr = format!("{ip}:{port}");
    debug!("Connecting to {}", addr);

    let mut stream = timeout(REQUEST_TIMEOUT, TcpStream::connect(&addr))
        .await
        .map_err(|e| format!("Connection to {addr} timed out: {e}"))?
        .map_err(|e| format!("TCP connect error to {addr}: {e}"))?;

    let mut buf = vec![0; 1024];
    let n = execute_tcp_shutdown_request(&mut stream, message.as_bytes(), &mut buf).await?;

    let Some(data) = buf.get(..n) else {
        unreachable!("Read data size should always be valid, as its >= buffer size");
    };
    Ok(String::from_utf8_lossy(data).to_string())
}

/// Send a shutdown command to a host.
async fn shutdown_host(host: &str, state: &AppState) -> Result<(), (StatusCode, &'static str)> {
    debug!("Attempting to shutdown host '{}'", host);

    let host_config = {
        let config = state.config_rx.borrow();
        match config.hosts.get(host) {
            Some(config) => {
                debug!(
                    "Found configuration for host '{}': ip={}, port={}",
                    host, config.ip, config.port
                );
                config.clone()
            }
            None => {
                error!("No configuration found for host '{}'", host);
                return Err((StatusCode::NOT_FOUND, "Unknown host"));
            }
        }
    };

    info!(
        "Sending shutdown command to '{}' ({}:{})",
        host, host_config.ip, host_config.port
    );
    let signed_message = create_signed_message("shutdown", &host_config.shared_secret);
    send_shutdown(&host_config.ip, host_config.port, &signed_message)
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
