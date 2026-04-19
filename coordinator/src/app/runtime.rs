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
    time::{Instant, MissedTickBehavior, interval, timeout_at},
};
use tracing::{debug, error, info, warn};

use shuthost_common::{
    BroadcastMessage, HmacValidationResult, create_signed_message, parse_hmac_message,
    protocol::{InitSystem, OsType},
    validate_hmac_message,
};

use super::state::{ConfigTx, HostInstallInfo, HostState, HostStatus, HostStatusState};
use crate::{
    app::{
        AppState, LeaseMapRaw, LeaseRx, WsTx, config_watcher::watch_config_file, db,
        host_control::spawn_handle_host_state,
    },
    config::Host,
    http::push,
    websocket::WsMessage,
};

use crate::app::host_control::HostWithName;

/// How long a diverged enforced-host state must be stable before the enforcer
/// re-triggers a wake / shutdown (prevents hammering during transitions).
pub const ENFORCE_STABILIZATION_THRESHOLD: Duration = Duration::from_secs(5);

/// Poll a single host for its online status.
async fn poll_host_status(host: &HostWithName) -> (HostState, Option<HostInstallInfo>) {
    let addr = format!("{}:{}", host.host.ip, host.host.port);
    let deadline = Instant::now() + Duration::from_millis(900);

    let Ok(Ok(mut stream)) = timeout_at(deadline, TcpStream::connect(&addr)).await else {
        return (HostState::Offline, None);
    };

    let signed_message = create_signed_message("status", host.host.shared_secret.as_ref());
    if let Err(e) = stream.write_all(signed_message.as_bytes()).await {
        debug!("Failed to write to {}: {}", host.name, e);
        return (HostState::Offline, None);
    }

    let mut buf = vec![0u8; 256];
    let Ok(Ok(n)) = timeout_at(deadline, stream.read(&mut buf)).await else {
        return (HostState::Offline, None);
    };

    let resp = String::from_utf8_lossy(buf.get(..n).expect("n <= buf.len() by definition"));
    // Accept any non-error response as online
    if resp.contains("ERROR") {
        (HostState::Offline, None)
    } else {
        (HostState::Online, parse_install_info(&resp))
    }
}

fn parse_install_info(resp: &str) -> Option<HostInstallInfo> {
    const PREFIX: &str = "OK: status";
    let resp = resp.trim();
    let suffix = resp.strip_prefix(PREFIX)?.trim_start();
    let suffix = suffix.strip_prefix(';')?.trim();
    if suffix.is_empty() {
        return None;
    }
    let mut agent_version = None;
    let mut init_system = None;
    let mut os = None;
    for section in suffix.split(';') {
        let section = section.trim();
        if let Some(v) = section.strip_prefix("agent_version=") {
            if !v.is_empty() {
                agent_version = Some(v.to_string());
            }
        } else if let Some(v) = section.strip_prefix("init_system=") {
            init_system = v.parse::<InitSystem>().ok();
        } else if let Some(v) = section.strip_prefix("os=") {
            os = v.parse::<OsType>().ok();
        }
    }
    Some(HostInstallInfo {
        agent_version,
        init_system,
        os,
    })
}

async fn maybe_update_host_install_info(
    state: &AppState,
    hostname: &str,
    agent_version: String,
    init_system: InitSystem,
    os: OsType,
) {
    let new_info = HostInstallInfo {
        agent_version: Some(agent_version.clone()),
        init_system: Some(init_system),
        os: Some(os),
    };
    let mut info_map = state.host_install_info.write().await;
    let current = info_map.get(hostname);
    let unchanged = current.is_some_and(|i| {
        i.agent_version.as_deref() == Some(&agent_version)
            && i.init_system == Some(init_system)
            && i.os == Some(os)
    });
    if unchanged {
        return;
    }

    info_map.insert(hostname.to_string(), new_info);
    drop(info_map);

    if let &Some(ref pool) = &state.db_pool
        && let Err(e) = db::upsert_host_install_info(
            pool.clone(),
            hostname.to_string(),
            agent_version,
            init_system,
            os,
        )
        .await
    {
        error!(host = %hostname, "Failed to persist host install info: {e:#}");
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
}

pub(super) async fn poll_until_host_state(
    host: &HostWithName,
    desired_state: HostState,
    deadline: Instant,
    poll_interval_ms: u64,
    hoststatus: &HostStatusState,
) -> Result<(), PollError> {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let (current_state, _) = poll_host_status(host).await;
        let tick_fut = ticker.tick();
        if current_state == desired_state {
            // Transition complete: write definitive stable state, clearing Waking/ShuttingDown.
            hoststatus.force_set(&host.name, current_state).await;
            return Ok(());
        }
        tick_fut.await; // wait for next tick before polling again
        if Instant::now() >= deadline {
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
        let mut hoststatus_rx = state.hoststatus.subscribe();
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
        let mut hoststatus_rx = state.hoststatus.subscribe();
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

    // Persist last-online timestamps when a host transitions to Online.
    if let Some(pool) = state.db_pool.clone() {
        let mut hoststatus_rx = state.hoststatus.subscribe();
        tasks.spawn(async move {
            let mut prev = hoststatus_rx.borrow().clone();
            while hoststatus_rx.changed().await.is_ok() {
                let current = hoststatus_rx.borrow().clone();
                for (host, h_state) in current.iter() {
                    if prev.get(host) != Some(h_state) && *h_state == HostState::Online {
                        let pool = pool.clone();
                        let host = host.clone();
                        tokio::spawn(async move {
                            if let Err(e) = db::upsert_host_last_online(pool, host.clone()).await {
                                error!(host = %host, "Failed to upsert host last_online: {e:#}");
                            }
                        });
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

    // Don't trigger while a control task is already in-flight.
    if current_state.is_transitioning() {
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
                    host_with_name.name, host_with_name.host.ip, host_with_name.host.port, polled.0
                );
                (name, polled)
            }
        });

        let results = future::join_all(futures).await;

        // Update install info from poll results.
        for &(ref host_name, (_, ref install_info)) in &results {
            if let Some(info) = install_info.clone()
                && let (Some(version), Some(init_system), Some(os)) =
                    (info.agent_version, info.init_system, info.os)
            {
                maybe_update_host_install_info(&state, host_name, version, init_system, os).await;
            }
        }

        // Apply polled states to the status map, skipping hosts in transition.
        let poll_iter = results
            .iter()
            .map(|&(ref name, (polled_state, _))| (name.as_str(), polled_state));
        if let Some((old_status, new_status)) = state.hoststatus.apply_poll_results(poll_iter).await
        {
            // Record timestamps for changed hosts (used by the enforce stabilisation timer).
            for (host, new_state) in new_status.iter() {
                if old_status.get(host) != Some(new_state) {
                    state_timestamps.insert(host.clone(), poll_start);
                }
            }

            // Fire push notifications for unscheduled state transitions.
            if let (Some(pool), Some(vapid_key)) = (state.db_pool.clone(), state.vapid_key.clone())
            {
                let leases_snapshot = state.leases.snapshot();
                spawn_push_notifications_for_unscheduled(
                    &old_status,
                    &new_status,
                    &leases_snapshot,
                    &pool,
                    &vapid_key,
                );
            }
        } else {
            debug!("No change in host status");
        }

        // Enforce state for hosts that opt in, after a stabilization delay.
        let current_status = state.hoststatus.borrow().clone();
        let leases_snapshot = state.leases.snapshot();
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
                spawn_handle_host_state(host_name, &state);
            }
        }

        ticker.tick().await;
    }
}

/// Spawns push-notification tasks for unscheduled host state transitions.
///
/// An event is considered unscheduled when:
/// - A host transitions `Offline → Online` with no active leases (`ShutHost` did not wake it up).
/// - A host transitions `Online → Offline` while leases are held (`ShutHost` did not shut it down).
fn spawn_push_notifications_for_unscheduled(
    old_status: &HostStatus,
    new_status: &HostStatus,
    leases: &Arc<LeaseMapRaw>,
    pool: &db::DbPool,
    vapid_key: &Arc<web_push::PartialVapidSignatureBuilder>,
) {
    for (host_name, &new_state) in new_status {
        let old_state = old_status
            .get(host_name)
            .copied()
            .unwrap_or(HostState::Offline);
        if old_state == new_state {
            continue;
        }
        let has_leases = leases.get(host_name).is_some_and(|s| !s.is_empty());
        let is_unscheduled = match (old_state, new_state) {
            (HostState::Offline, HostState::Online) => !has_leases,
            (HostState::Online, HostState::Offline) => has_leases,
            _ => false,
        };
        if !is_unscheduled {
            continue;
        }
        let body = match new_state {
            HostState::Online => format!("{host_name} started up unexpectedly"),
            HostState::Offline => format!("{host_name} shut down unexpectedly"),
            HostState::Waking | HostState::ShuttingDown => continue,
        };
        let pool = pool.clone();
        let vapid_key = vapid_key.clone();
        let host_name = host_name.clone();
        tokio::spawn(async move {
            match db::get_subscriptions_for_host_unscheduled(&pool, &host_name).await {
                Ok(subs) if !subs.is_empty() => {
                    let payload = serde_json::json!({
                        "title": "ShutHost",
                        "body": body,
                        "data": { "hostname": host_name },
                    })
                    .to_string();
                    push::send_push_notifications(&vapid_key, &pool, &subs, &payload).await;
                }
                Ok(_) => {}
                Err(e) => {
                    error!(host = %host_name, "Failed to fetch push subscriptions: {e:#}");
                }
            }
        });
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
    fn get_hosts_desired_offline(leases: &LeaseMapRaw) -> HashSet<String> {
        leases
            .iter()
            .filter(|&(_, lease_set)| lease_set.is_empty())
            .map(|(host, _)| host.clone())
            .collect()
    }

    let mut prev_desired_offline = get_hosts_desired_offline(&leases_rx.borrow_and_update());

    while leases_rx.changed().await.is_ok() {
        let new_leases = leases_rx.borrow_and_update();
        let new_desired_offline = get_hosts_desired_offline(&new_leases);
        let hoststatus = state.hoststatus.borrow();

        let changed_desired_state: HashSet<_> = prev_desired_offline
            .symmetric_difference(&new_desired_offline)
            .collect();

        for host_name in changed_desired_state {
            let desired_running = !new_desired_offline.contains(host_name);

            let current_state = *hoststatus.get(host_name).unwrap_or(&HostState::Offline);

            // Skip hosts already in a transition — the in-flight task re-checks on completion.
            if current_state.is_transitioning() {
                continue;
            }

            let is_running = current_state == HostState::Online;

            let needs_action = desired_running != is_running;

            if needs_action {
                spawn_handle_host_state(host_name, &state);
            }
        }

        prev_desired_offline = new_desired_offline;
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
    let bound_port = socket.local_addr().map_or(0, |a| a.port());
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

    state
        .hoststatus
        .force_set(hostname, HostState::Online)
        .await;

    maybe_update_host_install_info(
        state,
        hostname,
        startup.agent_version.clone(),
        startup.init_system,
        startup.os,
    )
    .await;
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
            wake_timeout_secs: None,
            shutdown_timeout_secs: None,
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

    #[test]
    fn parse_install_info_accepts_extended_status() {
        assert!(parse_install_info("OK: status").is_none());
        assert_eq!(
            parse_install_info("OK: status;agent_version=v1.2.3").map(|i| i.agent_version),
            Some(Some("v1.2.3".to_string()))
        );
        assert_eq!(
            parse_install_info("OK: status;agent_version=v1.2.3; init_system=systemd; os=linux")
                .map(|i| (i.agent_version, i.init_system, i.os)),
            Some((
                Some("v1.2.3".to_string()),
                Some(InitSystem::Systemd),
                Some(OsType::Linux)
            ))
        );
        assert_eq!(
            parse_install_info("OK: status;agent_version=").map(|i| i.agent_version),
            Some(None)
        );
        assert_eq!(
            parse_install_info("OK: status;other=1").map(|i| i.agent_version),
            Some(None)
        );
    }
}
