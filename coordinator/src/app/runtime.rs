//! Background polling tasks for the coordinator.

use alloc::sync::Arc;
use core::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use futures::future;
use thiserror::Error as ThisError;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpStream, UdpSocket},
    task::JoinSet,
    time::{Instant, MissedTickBehavior, interval, timeout},
};
use tracing::{debug, error, info, warn};

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

use crate::app::host_control::HostWithName;

/// How long a diverged enforced-host state must be stable before the enforcer
/// re-triggers a wake / shutdown (prevents hammering during transitions).
pub const ENFORCE_STABILIZATION_THRESHOLD: Duration = Duration::from_secs(5);

/// Poll a single host for its online status.
async fn poll_host_status(host: &HostWithName) -> HostState {
    let addr = format!("{}:{}", host.host.ip, host.host.port);
    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(mut stream)) => {
            let signed_message = create_signed_message("status", host.host.shared_secret.as_ref());
            if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
                debug!("Failed to write to {}: {}", host.name, e);
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
    host: &HostWithName,
    desired_state: HostState,
    timeout_secs: u64,
    poll_interval_ms: u64,
    hoststatus_tx: &HostStatusTx,
) -> Result<(), PollError> {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let start = Instant::now();
    loop {
        let poll_fut = poll_host_status(host);
        let tick_fut = ticker.tick();
        let (current_state, _) = tokio::join!(poll_fut, tick_fut);
        // Update global state
        let mut status_map = hoststatus_tx.borrow().as_ref().clone();
        if status_map.get(host.name.as_str()) != Some(&current_state) {
            status_map.insert(host.name.clone(), current_state);
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
                host_name: host.name.clone(),
                desired_state,
            });
        }
    }
}

/// Start all background tasks for the HTTP server.
/// Returns a [`JoinSet`] that owns all spawned tasks; dropping it aborts them all.
pub(super) fn start_background_tasks(
    state: &AppState,
    config_tx: &ConfigTx,
    config_path: &Path,
    broadcast_socket: UdpSocket,
) -> JoinSet<()> {
    let mut tasks = JoinSet::new();

    // Start host status polling task
    {
        let state = state.clone();
        tasks.spawn(async move {
            poll_host_statuses(state).await;
        });
    }

    // Start config file watcher
    {
        let path = config_path.to_path_buf();
        let config_tx = config_tx.clone();
        tasks.spawn(async move {
            watch_config_file(path, config_tx).await;
        });
    }

    // Forwards host status updates to the websocket client loops
    {
        let ws_tx = state.ws_tx.clone();
        let mut hoststatus_rx = state.hoststatus_tx.subscribe();
        tasks.spawn(async move {
            while hoststatus_rx.changed().await.is_ok() {
                let msg = WsMessage::HostStatus(hoststatus_rx.borrow().as_ref().clone());
                if ws_tx.send(msg).is_err() {
                    debug!("No Websocket Subscribers");
                }
            }
        });
    }

    // Log host state transitions.
    {
        let mut hoststatus_rx = state.hoststatus_tx.subscribe();
        tasks.spawn(async move {
            let mut prev = hoststatus_rx.borrow().clone();
            while hoststatus_rx.changed().await.is_ok() {
                let current = hoststatus_rx.borrow().clone();
                for (host, h_state) in current.iter() {
                    if prev.get(host) != Some(h_state) {
                        info!(host = %host, state = ?h_state, "Host status changed");
                    }
                }
                prev = current;
            }
        });
    }

    // Forwards config changes to the websocket client loops
    {
        let ws_tx = state.ws_tx.clone();
        let mut config_rx = state.config_rx.clone();
        tasks.spawn(async move {
            while config_rx.changed().await.is_ok() {
                let config = config_rx.borrow();
                let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
                let clients = config.clients.keys().cloned().collect::<Vec<_>>();
                let msg = WsMessage::ConfigChanged { hosts, clients };
                if ws_tx.send(msg).is_err() {
                    debug!("No Websocket Subscribers");
                }
            }
        });
    }

    // Reconcile host state on lease changes (edge-triggered, all hosts)
    {
        let leases_rx = state.leases.subscribe();
        let state = state.clone();
        tasks.spawn(async move {
            reconcile_on_lease_change(leases_rx, state).await;
        });
    }

    // Forwards per-host lease changes to the websocket client loops
    {
        let leases_rx = state.leases.subscribe();
        let ws_tx = state.ws_tx.clone();
        tasks.spawn(async move {
            broadcast_lease_updates(leases_rx, ws_tx).await;
        });
    }

    // Listens for UDP startup broadcasts from agents and persists IP overrides.
    {
        let state = state.clone();
        tasks.spawn(async move {
            listen_for_agent_startup(state, broadcast_socket).await;
        });
    }

    tasks
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
            let mut host_clone = host.clone();
            let (ip, port) = ip_overrides.get(name.as_str()).map_or_else(
                || (host_clone.ip.clone(), host_clone.port),
                |&(ref ip, port)| (ip.clone(), port),
            );
            host_clone.ip = ip;
            host_clone.port = port;
            let host_with_name = HostWithName {
                name: name.clone(),
                host: host_clone,
            };
            async move {
                let polled = poll_host_status(&host_with_name).await;
                debug!(
                    "Polled {} at {}:{} - state: {:?}",
                    host_with_name.name, host_with_name.host.ip, host_with_name.host.port, polled
                );
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

/// Background task: listens on the pre-bound UDP socket for agent startup announcements.
/// When a valid signed broadcast is received, the host is immediately marked Online and any
/// IP/port differences are persisted as overrides.
///
/// The socket is bound once at startup. `broadcast_port` changes in the config file are never
/// propagated at runtime (the config watcher only applies `[hosts]` and `[clients]` changes),
/// so no port-change handling is needed here.
async fn listen_for_agent_startup(state: AppState, socket: UdpSocket) {
    let bound_port = socket.local_addr().map(|a| a.port()).unwrap_or(0);
    let mut buf = vec![0u8; 4096];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((n, peer_addr)) => {
                let data = buf
                    .get(..n)
                    .expect("n should be <= buf.size by definition")
                    .to_vec();
                handle_startup_packet(&data, peer_addr, &state).await;
            }
            Err(e) => {
                error!("UDP receive error on port {bound_port}: {e}");
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

    let Some(startup) = parse_startup_broadcast(raw, peer_addr) else {
        return;
    };

    let hostname = &startup.hostname;
    let Some(host_cfg) = lookup_host_config(state, hostname, peer_addr) else {
        return;
    };

    if !validate_startup_hmac(raw, &host_cfg, peer_addr, hostname) {
        return;
    }

    info!("Received valid startup broadcast from host '{hostname}' at {peer_addr}");

    mark_host_online(state, hostname);
    persist_host_override_if_needed(state, hostname, &host_cfg, &startup).await;
}

fn parse_startup_broadcast(
    raw: &str,
    peer_addr: SocketAddr,
) -> Option<shuthost_common::StartupBroadcast> {
    // The signed message format is "timestamp|{json}|signature".
    // We extract the JSON so we can look up the host's secret before doing full HMAC validation.
    let Some((_, json_payload, _)) = parse_hmac_message(raw) else {
        debug!("Malformed startup packet from {peer_addr}");
        return None;
    };

    match serde_json::from_str::<BroadcastMessage>(&json_payload) {
        Ok(BroadcastMessage::AgentStartup(startup)) => Some(startup),
        Err(e) => {
            debug!("Failed to parse startup broadcast JSON from {peer_addr}: {e}");
            None
        }
    }
}

fn lookup_host_config(state: &AppState, hostname: &str, peer_addr: SocketAddr) -> Option<Host> {
    let config = state.config_rx.borrow().clone();
    match config.hosts.get(hostname).cloned() {
        Some(cfg) => Some(cfg),
        None => {
            debug!("Startup broadcast for unknown host '{hostname}' from {peer_addr}, ignoring");
            None
        }
    }
}

fn validate_startup_hmac(
    raw: &str,
    host_cfg: &Host,
    peer_addr: SocketAddr,
    hostname: &str,
) -> bool {
    let mac_is_valid = matches!(
        validate_hmac_message(raw, &host_cfg.shared_secret),
        HmacValidationResult::Valid(_)
    );
    if !mac_is_valid {
        debug!("Invalid HMAC on startup broadcast from {peer_addr} claiming to be '{hostname}'");
    }
    mac_is_valid
}

fn mark_host_online(state: &AppState, hostname: &str) {
    let mut status_map = state.hoststatus_tx.borrow().as_ref().clone();
    if status_map.get(hostname) != Some(&HostState::Online) {
        status_map.insert(hostname.to_string(), HostState::Online);
        if state.hoststatus_tx.send(Arc::new(status_map)).is_err() {
            debug!("Host status channel closed");
        }
    }
}

async fn persist_host_override_if_needed(
    state: &AppState,
    hostname: &str,
    host_cfg: &Host,
    startup: &shuthost_common::StartupBroadcast,
) {
    let agent_ip = &startup.ip_address;
    let agent_port = startup.port;

    // Validate the agent-reported IP address before trusting/persisting it.
    let agent_ip_trimmed = agent_ip.trim();
    let parsed_ip = agent_ip_trimmed.parse::<IpAddr>();
    if let Err(e) = parsed_ip {
        warn!(
            "Ignoring invalid agent IP address '{}' for host '{}': {e}",
            agent_ip, hostname
        );
        return;
    }

    if agent_ip != &host_cfg.ip || agent_port != host_cfg.port {
        warn!(
            "Host '{hostname}' address differs from config: config={}:{}, agent={}:{}; storing override",
            host_cfg.ip, host_cfg.port, agent_ip, agent_port
        );

        {
            let mut overrides = state.host_overrides.write().await;
            overrides.insert(
                hostname.to_string(),
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
    } else {
        // The agent-reported address matches the static config again.
        // Clear any existing override from memory and the database.
        let mut removed_override = false;
        {
            let mut overrides = state.host_overrides.write().await;
            if overrides.remove(hostname).is_some() {
                removed_override = true;
            }
        }

        if removed_override
            && let Some(ref pool) = state.db_pool
            && let Err(e) = db::delete_host_ip_override(pool, hostname).await
        {
            error!("Failed to clear IP override for '{hostname}': {e}");
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
