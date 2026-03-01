//! Background polling tasks for the coordinator.

use alloc::sync::Arc;
use core::{net::SocketAddr, time::Duration};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use futures::future;
use thiserror::Error as ThisError;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpStream, UdpSocket},
    time::{Instant, MissedTickBehavior, interval, sleep, timeout},
};
use tracing::{Instrument as _, debug, error, info, warn};

use shuthost_common::{
    BroadcastMessage, HmacValidationResult, create_signed_message, parse_hmac_message,
    validate_hmac_message,
};

use super::state::{ConfigTx, HostState, HostStatusTx};
use crate::{
    app::{
        AppState, LeaseMapRaw, LeaseRx, WsTx, config_watcher::watch_config_file, db,
        host_control::spawn_handle_host_state,
    },
    config::Host,
    websocket::WsMessage,
};

/// How long a diverged enforced-host state must be stable before the enforcer
/// re-triggers a wake / shutdown (prevents hammering during transitions).
pub const ENFORCE_STABILIZATION_THRESHOLD: Duration = Duration::from_secs(5);

/// Poll a single host for its online status.
async fn poll_host_status(
    name: &str,
    ip: &str,
    port: u16,
    shared_secret: &secrecy::SecretString,
) -> HostState {
    let addr = format!("{ip}:{port}");
    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(mut stream)) => {
            let signed_message = create_signed_message("status", shared_secret);
            if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                debug!("Failed to write to {}: {}", name, e);
                return HostState::Offline;
            }
            let mut buf = vec![0u8; 256];
            match timeout(Duration::from_millis(400), stream.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    let Some(data) = buf.get(..n) else {
                        unreachable!(
                            "Read data size should always be valid, as its >= buffer size"
                        );
                    };
                    let resp = String::from_utf8_lossy(data);
                    // Accept any non-error response as online
                    if resp.contains("ERROR") {
                        HostState::Offline
                    } else {
                        HostState::Online
                    }
                }
                _ => HostState::Offline,
            }
        }
        _ => HostState::Offline,
    }
}

/// Poll a host until its state matches `desired_state` or timeout is reached. Updates global state.
///
/// # Errors
///
/// Returns an error if the polling times out or if there are issues with the host configuration.
#[derive(Debug, ThisError)]
pub(super) enum PollError {
    #[error("Timeout waiting for host '{host_name}' to become {desired_state:?}")]
    Timeout {
        host_name: String,
        desired_state: HostState,
    },
    #[error("Coordinator shutting down")]
    CoordinatorShuttingDown,
}

pub(super) async fn poll_until_host_state(
    host_name: &str,
    host: &Host,
    desired_state: HostState,
    timeout_secs: u64,
    poll_interval_ms: u64,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), PollError> {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let start = Instant::now();
    loop {
        let poll_fut =
            poll_host_status(host_name, &host.ip, host.port, host.shared_secret.as_ref());
        let tick_fut = ticker.tick();
        let (current_state, _) = tokio::join!(poll_fut, tick_fut);
        // Update global state
        let mut status_map = hoststatus_tx.borrow().as_ref().clone();
        if status_map.get(host_name) != Some(&current_state) {
            status_map.insert(host_name.to_string(), current_state);
            if hoststatus_tx.send(Arc::new(status_map)).is_err() {
                debug!("Host status receiver dropped, stopping polling");
                return Err(PollError::CoordinatorShuttingDown);
            }
        }
        if current_state == desired_state {
            return Ok(());
        }
        if start.elapsed().as_secs() >= timeout_secs {
            return Err(PollError::Timeout {
                host_name: host_name.to_string(),
                desired_state,
            });
        }
    }
}

/// Start all background tasks for the HTTP server.
pub(super) fn start_background_tasks(state: &AppState, config_tx: &ConfigTx, config_path: &Path) {
    // Start host status polling task
    {
        let state = state.clone();
        tokio::spawn(
            async move {
                poll_host_statuses(state).await;
            }
            .in_current_span(),
        );
    }

    // Start config file watcher
    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tokio::spawn(
            async move {
                watch_config_file(path, config_tx).await;
            }
            .in_current_span(),
        );
    }

    // Forwards host status updates to the websocket client loops
    {
        let ws_tx = state.ws_tx.clone();
        let mut hoststatus_rx = state.hoststatus_tx.subscribe();
        tokio::spawn(
            async move {
                while hoststatus_rx.changed().await.is_ok() {
                    let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                    if ws_tx.send(msg).is_err() {
                        debug!("No Websocket Subscribers");
                    }
                }
            }
            .in_current_span(),
        );
    }

    // Forwards config changes to the websocket client loops
    {
        let ws_tx = state.ws_tx.clone();
        let mut config_rx = state.config_rx.clone();
        tokio::spawn(
            async move {
                while config_rx.changed().await.is_ok() {
                    let config = config_rx.borrow();
                    let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
                    let clients = config.clients.keys().cloned().collect::<Vec<_>>();
                    let msg = WsMessage::ConfigChanged { hosts, clients };
                    if ws_tx.send(msg).is_err() {
                        debug!("No Websocket Subscribers");
                    }
                }
            }
            .in_current_span(),
        );
    }

    // Reconcile host state on lease changes (edge-triggered, all hosts)
    {
        let leases_rx = state.leases.subscribe();
        let state = state.clone();
        tokio::spawn(
            async move {
                reconcile_on_lease_change(leases_rx, state).await;
            }
            .in_current_span(),
        );
    }

    // Forwards per-host lease changes to the websocket client loops
    {
        let leases_rx = state.leases.subscribe();
        let ws_tx = state.ws_tx.clone();
        tokio::spawn(
            async move {
                broadcast_lease_updates(leases_rx, ws_tx).await;
            }
            .in_current_span(),
        );
    }

    // Listens for UDP startup broadcasts from agents and persists IP overrides.
    {
        let state = state.clone();
        tokio::spawn(
            async move {
                listen_for_agent_startup(state).await;
            }
            .in_current_span(),
        );
    }
}

/// Determine whether the given host configuration and observed runtime state
/// warrant spawning a control task to enforce the desired state.
///
/// * `host_cfg` - the configuration for the host, which contains the
///   `enforce_state` flag.
/// * `lease_set` - the set of active lease holders for the host; non-empty means
///   the host should be running.
/// * `current_state` - the most recently observed state of the host.
/// * `stable_for` - how long the last state transition has been stable.
///
/// Returns `true` if an action should be spawned. Note that callers are
/// responsible for applying the stabilization threshold and actually spawning a
/// task.
fn should_enforce_action(
    host_cfg: &Host,
    lease_set: &super::host_control::LeaseSources,
    current_state: HostState,
    stable_for: Duration,
) -> bool {
    if !host_cfg.enforce_state {
        return false;
    }

    let desired_running = !lease_set.is_empty();
    let is_running = current_state == HostState::Online;
    let needs_action = (desired_running && !is_running) || (!desired_running && is_running);

    needs_action && stable_for >= ENFORCE_STABILIZATION_THRESHOLD
}

/// Background task: periodically polls each host for status by attempting a TCP connection and HMAC ping.
/// For hosts with `enforce_state = true`, also re-triggers control if the actual state diverges from
/// the lease-implied desired state (after a stabilization delay).
///
/// The logic determining whether an enforcement action should be triggered is
/// factored into `should_enforce_action` which makes it easy to unit test.
async fn poll_host_statuses(state: AppState) {
    let poll_interval = Duration::from_secs(2);
    let mut ticker = interval(poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // Tracks when each host's state last changed (to enforce stability when updates come in from multiple sources).
    let mut state_timestamps: HashMap<String, Instant> = HashMap::new();

    loop {
        let poll_start = Instant::now();
        let config = state.config_rx.borrow().clone();

        // Read IP/port overrides once per poll cycle into an owned map so the
        // read-guard is dropped before the async join_all below.
        let ip_overrides: HashMap<String, (String, u16)> = {
            let overrides = state.host_overrides.read().await;
            overrides
                .iter()
                .map(|(k, v)| (k.clone(), (v.ip.clone(), v.port)))
                .collect()
        };

        let futures = config.hosts.iter().map(|(name, host)| {
            let name = name.clone();
            let (ip, port) = ip_overrides.get(name.as_str()).map_or_else(
                || (host.ip.clone(), host.port),
                |&(ref ip, port)| (ip.clone(), port),
            );
            let shared_secret = host.shared_secret.clone();
            async move {
                let polled = poll_host_status(&name, &ip, port, shared_secret.as_ref()).await;
                debug!("Polled {} at {}:{} - state: {:?}", name, ip, port, polled);
                (name, polled)
            }
        });

        let results = future::join_all(futures).await;

        // Update the status map, recording the poll_start timestamp for any state changes.
        let old_status = state.hoststatus_tx.borrow().clone();
        let mut new_status = (*old_status).clone();
        let mut any_changed = false;

        for (host_name, new_state) in results {
            if old_status.get(&host_name) != Some(&new_state) {
                new_status.insert(host_name.clone(), new_state);
                state_timestamps.insert(host_name, poll_start);
                any_changed = true;
            }
        }

        if any_changed {
            info!("Host status changed: {:?}", new_status);
            if state.hoststatus_tx.send(Arc::new(new_status)).is_err() {
                debug!("Host status receiver dropped, stopping polling");
                break;
            }
        } else {
            debug!("No change in host status");
        }

        // Enforce state for hosts that opt in, after a stabilization delay.
        let current_status = state.hoststatus_tx.borrow().clone();
        let leases_snapshot = state.leases.borrow().clone();
        for (host_name, host_cfg) in &config.hosts {
            let lease_set = leases_snapshot.get(host_name).cloned().unwrap_or_default();
            let current_state = current_status
                .get(host_name)
                .copied()
                .unwrap_or(HostState::Offline);

            let stable_for = state_timestamps
                .get(host_name)
                .map_or(ENFORCE_STABILIZATION_THRESHOLD, Instant::elapsed);

            if should_enforce_action(host_cfg, &lease_set, current_state, stable_for) {
                spawn_handle_host_state(host_name, &lease_set, &state);
            }
        }

        ticker.tick().await;
    }
}

/// Background task: forwards per-host lease changes to WebSocket clients.
async fn broadcast_lease_updates(mut leases_rx: LeaseRx, ws_tx: WsTx) {
    let mut prev_leases: Arc<LeaseMapRaw> = leases_rx.borrow_and_update().clone();

    while leases_rx.changed().await.is_ok() {
        let new_leases: Arc<LeaseMapRaw> = leases_rx.borrow_and_update().clone();

        // Collect all host names that appear in either snapshot.
        let all_hosts: HashSet<&str> = prev_leases
            .keys()
            .chain(new_leases.keys())
            .map(String::as_str)
            .collect();

        for host in all_hosts {
            if prev_leases.get(host) != new_leases.get(host) {
                let leases = new_leases.get(host).cloned().unwrap_or_default();
                let msg = WsMessage::LeaseUpdate {
                    host: host.to_string(),
                    leases,
                };
                if ws_tx.send(msg).is_err() {
                    debug!("No Websocket Subscribers");
                }
            }
        }

        prev_leases = new_leases;
    }
}

/// Background task: reconcile host control on every lease-map change (edge-triggered, all hosts).
async fn reconcile_on_lease_change(mut leases_rx: LeaseRx, state: AppState) {
    fn get_hosts_desired_online(leases: &LeaseMapRaw) -> HashSet<String> {
        leases
            .iter()
            .filter(|&(_, lease_set)| lease_set.is_empty())
            .map(|(host, _)| host.clone())
            .collect()
    }

    let mut prev_desired_online = get_hosts_desired_online(&leases_rx.borrow_and_update());

    while leases_rx.changed().await.is_ok() {
        let new_leases = leases_rx.borrow_and_update();
        let new_desired_online = get_hosts_desired_online(&new_leases);
        let hoststatus = state.hoststatus_tx.borrow();

        let changed_desired_state: HashSet<_> = prev_desired_online
            .symmetric_difference(&new_desired_online)
            .collect();

        for host_name in changed_desired_state {
            let empty = HashSet::new();

            let lease_set = new_leases.get(host_name).unwrap_or(&empty);

            let desired_running = !new_desired_online.contains(host_name);

            let current_state = *hoststatus.get(host_name).unwrap_or(&HostState::Offline);

            let is_running = current_state == HostState::Online;

            let needs_action = desired_running != is_running;

            if needs_action {
                spawn_handle_host_state(host_name, lease_set, &state);
            }
        }

        prev_desired_online = new_desired_online;
    }
}

/// Background task: listens on the configured UDP broadcast port for agent startup announcements.
/// When a valid signed broadcast is received, the host is immediately marked Online and any
/// IP/port differences are persisted as overrides.
async fn listen_for_agent_startup(state: AppState) {
    let mut config_rx = state.config_rx.clone();
    loop {
        let broadcast_port = config_rx.borrow_and_update().server.broadcast_port;
        let addr = format!("0.0.0.0:{broadcast_port}");
        let socket = match UdpSocket::bind(&addr).await {
            Ok(s) => {
                info!("Listening for agent startup broadcasts on {addr}");
                s
            }
            Err(e) => {
                error!("Failed to bind UDP broadcast socket on {addr}: {e}");
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut buf = vec![0u8; 4096];
        loop {
            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((n, peer_addr)) => {
                            let data = buf.get(..n).expect("n should be <= buf.size by definition").to_vec();
                            handle_startup_packet(&data, peer_addr, &state).await;
                        }
                        Err(e) => {
                            error!("UDP receive error on port {broadcast_port}: {e}");
                            break;
                        }
                    }
                }
                _ = config_rx.changed() => {
                    let new_port = config_rx.borrow().server.broadcast_port;
                    if new_port != broadcast_port {
                        info!("Broadcast port changed from {broadcast_port} to {new_port}, rebinding");
                        break;
                    }
                }
            }
        }
    }
}

/// Process a single UDP packet received on the broadcast port.
async fn handle_startup_packet(data: &[u8], peer_addr: SocketAddr, state: &AppState) {
    let Ok(raw) = str::from_utf8(data) else {
        debug!("Received non-UTF-8 startup packet from {peer_addr}, ignoring");
        return;
    };

    // The signed message format is "timestamp|{json}|signature".
    // We extract the JSON so we can look up the host's secret before doing full HMAC validation.
    let Some((_, json_payload, _)) = parse_hmac_message(raw) else {
        debug!("Malformed startup packet from {peer_addr}");
        return;
    };

    let broadcast: BroadcastMessage = match serde_json::from_str(&json_payload) {
        Ok(b) => b,
        Err(e) => {
            debug!("Failed to parse startup broadcast JSON from {peer_addr}: {e}");
            return;
        }
    };

    let BroadcastMessage::AgentStartup(ref startup) = broadcast;
    let hostname = &startup.hostname;

    let config = state.config_rx.borrow().clone();
    let Some(host_cfg) = config.hosts.get(hostname).cloned() else {
        debug!("Startup broadcast for unknown host '{hostname}' from {peer_addr}, ignoring");
        return;
    };

    // Full HMAC + timestamp validation using the host's shared secret.
    if !matches!(
        validate_hmac_message(raw, &host_cfg.shared_secret),
        HmacValidationResult::Valid(_)
    ) {
        warn!("Invalid HMAC on startup broadcast from {peer_addr} claiming to be '{hostname}'");
        return;
    }

    info!("Received valid startup broadcast from host '{hostname}' at {peer_addr}");

    // Immediately mark host Online to avoid waiting for the next poll cycle.
    {
        let mut status_map = state.hoststatus_tx.borrow().as_ref().clone();
        if status_map.get(hostname.as_str()) != Some(&HostState::Online) {
            status_map.insert(hostname.clone(), HostState::Online);
            if state.hoststatus_tx.send(Arc::new(status_map)).is_err() {
                debug!("Host status channel closed");
            }
        }
    }

    // If the agent's IP/port differs from the config, persist an override.
    let agent_ip = &startup.ip_address;
    let agent_port = startup.port;
    if agent_ip != &host_cfg.ip || agent_port != host_cfg.port {
        warn!(
            "Host '{hostname}' address differs from config: config={}:{}, agent={}:{}; storing override",
            host_cfg.ip, host_cfg.port, agent_ip, agent_port
        );
        {
            let mut overrides = state.host_overrides.write().await;
            overrides.insert(
                hostname.clone(),
                db::HostOverride {
                    ip: agent_ip.clone(),
                    port: agent_port,
                },
            );
        }
        if let Some(ref pool) = state.db_pool
            && let Err(e) = db::upsert_host_ip_override(pool, hostname, agent_ip, agent_port).await
        {
            error!("Failed to persist IP override for '{hostname}': {e}");
        }
    }
}

// -------------------------------------------------------------
// Unit tests for enforcement-related code
// -------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::host_control::{LeaseSource, LeaseSources};
    use alloc::sync::Arc;
    use core::time::Duration;
    use std::collections::HashSet;

    fn make_host(enforce: bool) -> Host {
        Host {
            ip: String::new(),
            mac: String::new(),
            port: 0,
            shared_secret: Arc::new(secrecy::SecretString::new(String::new().into())),
            enforce_state: enforce,
        }
    }

    #[test]
    fn should_enforce_respects_flag_and_state() {
        let cfg = make_host(false);
        let lease_set: LeaseSources = HashSet::new();

        // enforce_state disabled -> never trigger
        assert!(!should_enforce_action(
            &cfg,
            &lease_set,
            HostState::Offline,
            Duration::ZERO
        ));

        let cfg = make_host(true);
        // no mismatch: both offline
        assert!(!should_enforce_action(
            &cfg,
            &lease_set,
            HostState::Offline,
            Duration::from_secs(100)
        ));
        // mismatch but short stable time
        let lease_set: LeaseSources = vec![LeaseSource::WebInterface].into_iter().collect();
        assert!(!should_enforce_action(
            &cfg,
            &lease_set,
            HostState::Offline,
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn should_enforce_checks_threshold() {
        let cfg = make_host(true);
        let lease_set: LeaseSources = vec![LeaseSource::WebInterface].into_iter().collect();
        let current = HostState::Offline;
        assert!(!should_enforce_action(
            &cfg,
            &lease_set,
            current,
            ENFORCE_STABILIZATION_THRESHOLD
                .checked_sub(Duration::from_secs(1))
                .unwrap()
        ));
        assert!(should_enforce_action(
            &cfg,
            &lease_set,
            current,
            ENFORCE_STABILIZATION_THRESHOLD
        ));
    }
}
