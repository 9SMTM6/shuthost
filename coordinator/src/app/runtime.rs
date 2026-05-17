//! Background polling tasks for the coordinator.

use alloc::sync::Arc;
use core::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use std::collections::{HashMap, HashSet};

use futures::future;
use thiserror::Error as ThisError;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpStream, UdpSocket},
    sync::{
        RwLock,
        broadcast::{self, error::RecvError},
    },
    task::JoinSet,
    time::{Instant, MissedTickBehavior, interval, sleep, timeout_at},
};
use tracing::{debug, error, info, warn};
use web_push_native::jwt_simple::algorithms::ES256KeyPair;

use shuthost_common::{
    BroadcastMessage, HmacValidationResult, create_signed_message, parse_hmac_message,
    protocol::{InitSystem, OsType},
    validate_hmac_message,
};

use super::state::{ConfigRx, ConfigTx, HostInstallInfo, HostState, HostStatus};
use crate::{
    app::{
        AppState, HostActorHandle, LeaseMapRaw, LeaseRx, OperationFailureMap, WsTx,
        config_watcher::watch_config_file, db, host_actor::HostEvent,
        host_control::spawn_handle_host_state, shared_watch_store::SharedWatchRx,
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
    let mut script_path = None;
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
        } else if let Some(v) = section.strip_prefix("script_path=")
            && !v.is_empty()
        {
            script_path = Some(v.to_string());
        }
    }
    Some(HostInstallInfo {
        agent_version,
        init_system,
        os,
        script_path,
    })
}

async fn maybe_update_host_install_info(
    state: &AppState,
    hostname: &str,
    agent_version: String,
    init_system: InitSystem,
    os: OsType,
    script_path: Option<String>,
) {
    let new_info = HostInstallInfo {
        agent_version: Some(agent_version.clone()),
        init_system: Some(init_system),
        os: Some(os),
        script_path: script_path.clone(),
    };
    let mut info_map = state.host_install_info.write().await;
    let current = info_map.get(hostname);
    if current == Some(&new_info) {
        return;
    }

    info_map.insert(hostname.to_string(), new_info);
    drop(info_map);

    if let &Some(ref pool) = &state.db_pool {
        if let Err(e) = db::upsert_host_install_info(
            pool.clone(),
            hostname.to_string(),
            agent_version.clone(),
            init_system,
            os,
            script_path.clone(),
        )
        .await
        {
            error!(host = %hostname, "Failed to persist host install info: {e:#}");
        }

        if let Ok(mut host_stats) = db::get_all_host_stats(pool).await
            && let Some(mut stats) = host_stats.remove(hostname)
        {
            if state.host_actor.get_current_state(hostname) == HostState::Online {
                stats.is_online = true;
            }
            if let Err(_err) = state.ws_tx.send(WsMessage::HostStats {
                host: hostname.to_string(),
                stats,
            }) {
                debug!("No Websocket Subscribers for host stats");
            }
        }
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
) -> Result<(), PollError> {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let (current_state, _) = poll_host_status(host).await;
        let tick_fut = ticker.tick();
        if current_state == desired_state {
            // State reached: the caller is responsible for informing the actor
            // (via transition_complete or apply_poll_results).
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
    broadcast_socket: UdpSocket,
) -> JoinSet<()> {
    let mut tasks = JoinSet::new();

    tasks.spawn(watch_config_file(
        state.config_path.clone(),
        config_tx.clone(),
    ));

    // Reconcile host state on lease changes (edge-triggered, all hosts)
    tasks.spawn(reconcile_on_lease_change(
        state.leases.subscribe(),
        state.clone(),
    ));

    tasks.spawn(listen_for_agent_startup(state.clone(), broadcast_socket));

    spawn_websocket_forwarders(
        &mut tasks,
        &state.ws_tx,
        state.host_actor.subscribe_status(),
        state.operation_failures.subscribe(),
        state.config_rx.clone(),
        state.leases.subscribe(),
    );

    tasks.spawn(log_host_transitions(state.host_actor.subscribe_status()));

    tasks.spawn(persist_last_online(
        state.db_pool.clone(),
        state.host_actor.subscribe_status(),
    ));

    tasks.spawn(notify_for_online_durations(
        state.host_actor.subscribe_status(),
        state.online_since.clone(),
        state.db_pool.clone(),
        state.vapid_key.clone(),
    ));

    // Forward lease changes into the HostActor event stream.
    tasks.spawn(forward_lease_events(
        state.leases.subscribe(),
        state.host_actor.clone(),
    ));

    // Consume the HostEvent stream to fire unscheduled push notifications.
    tasks.spawn(handle_host_events(
        state.host_actor.subscribe_events(),
        state.leases.snapshot(),
        state.db_pool.clone(),
        state.vapid_key.clone(),
        state.leases.subscribe(),
    ));

    // Spawn this last since other tasks may depend on some changes triggered by this task, e.g. last-online.
    tasks.spawn(poll_host_statuses(state.clone()));

    tasks
}

fn spawn_websocket_forwarders(
    tasks: &mut JoinSet<()>,
    ws_tx: &WsTx,
    mut hoststatus_rx: SharedWatchRx<HostStatus>,
    mut op_failure_rx: SharedWatchRx<OperationFailureMap>,
    config_rx: ConfigRx,
    leases_rx: LeaseRx,
) {
    // Forwards host status updates to the websocket client loops
    let ws_tx_status = ws_tx.clone();
    let config_rx_for_status = config_rx.clone();
    tasks.spawn(async move {
        while hoststatus_rx.changed().await.is_ok() {
            let mut status_map = hoststatus_rx.borrow().as_ref().clone();
            let config = config_rx_for_status.borrow();
            for host in config.hosts.keys() {
                status_map.entry(host.clone()).or_insert(HostState::Offline);
            }
            let msg = WsMessage::HostStatus(status_map);
            if ws_tx_status.send(msg).is_err() {
                debug!("No Websocket Subscribers");
            }
        }
    });

    // Forwards operation failure state changes to websocket client loops
    let ws_tx_failure = ws_tx.clone();
    tasks.spawn(async move {
        while op_failure_rx.changed().await.is_ok() {
            let msg = WsMessage::OperationFailed(op_failure_rx.borrow().as_ref().clone());
            if ws_tx_failure.send(msg).is_err() {
                debug!("No Websocket Subscribers");
            }
        }
    });

    let mut config_rx = config_rx;
    let ws_tx_config = ws_tx.clone();
    tasks.spawn(async move {
        while config_rx.changed().await.is_ok() {
            let config = config_rx.borrow();
            let hosts = config.hosts.keys().cloned().collect::<Vec<_>>();
            let clients = config.clients.keys().cloned().collect::<Vec<_>>();
            let msg = WsMessage::ConfigChanged { hosts, clients };
            if ws_tx_config.send(msg).is_err() {
                debug!("No Websocket Subscribers");
            }
        }
    });

    let ws_tx_leases = ws_tx.clone();
    tasks.spawn(async move {
        broadcast_lease_updates(leases_rx, ws_tx_leases).await;
    });
}

async fn persist_last_online(
    db_pool: Option<db::DbPool>,
    mut hoststatus_rx: SharedWatchRx<HostStatus>,
) {
    if let Some(pool) = db_pool {
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
    }
}

async fn log_host_transitions(mut hoststatus_rx: SharedWatchRx<HostStatus>) {
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
}

async fn notify_for_online_durations(
    mut hoststatus_rx: SharedWatchRx<HostStatus>,
    online_since: Arc<RwLock<HashMap<String, Instant>>>,
    db_pool: Option<db::DbPool>,
    vapid_key: Option<Arc<ES256KeyPair>>,
) {
    let mut prev = hoststatus_rx.borrow().clone();
    while hoststatus_rx.changed().await.is_ok() {
        let current = hoststatus_rx.borrow().clone();
        for (host, h_state) in current.iter() {
            if prev.get(host) == Some(h_state) {
                continue;
            }
            match *h_state {
                HostState::Online => {
                    let now = Instant::now();
                    online_since.write().await.insert(host.clone(), now);

                    if let (Some(pool), Some(vapid_key)) = (db_pool.clone(), vapid_key.clone()) {
                        let host_clone = host.clone();
                        let online_since = online_since.clone();
                        tokio::spawn(async move {
                            spawn_online_for_notifications(
                                &host_clone,
                                now,
                                &online_since,
                                &pool,
                                &vapid_key,
                            )
                            .await;
                        });
                    }
                }
                HostState::Offline => {
                    online_since.write().await.remove(host);
                }
                HostState::Waking | HostState::ShuttingDown => {}
            }
        }
        prev = current;
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
    threshold: Duration,
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

    needs_action && stable_for >= threshold
}

/// Background task: periodically polls each host for status by attempting a TCP connection and HMAC ping.
/// For hosts with `enforce_state = true`, also re-triggers control if the actual state diverges from
/// the lease-implied desired state (after a stabilization delay).
///
/// The logic determining whether an enforcement action should be triggered is
/// factored into `should_enforce_action` which makes it easy to unit test.
async fn poll_host_statuses(state: AppState) {
    let poll_interval = Duration::from_secs(state.runtime.status_poll_interval_secs);
    let enforce_threshold = Duration::from_secs(state.runtime.enforce_stabilization_threshold_secs);
    let mut ticker = interval(poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // Tracks when each host's state last changed (to enforce stability when updates come in from multiple sources).
    let mut state_timestamps: HashMap<String, Instant> = HashMap::new();

    loop {
        let poll_start = Instant::now();
        let config = state.config_rx.borrow().clone();
        // Snapshot the current status before polling so we can detect changes.
        let pre_poll_status = state.host_actor.snapshot();

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
                maybe_update_host_install_info(
                    &state,
                    host_name,
                    version,
                    init_system,
                    os,
                    info.script_path,
                )
                .await;
            }
        }

        // Apply polled states to the actor, which will skip any host with an active control task.
        let poll_iter = results
            .iter()
            .map(|&(ref name, (ref polled_state, _))| (name.clone(), *polled_state));
        state.host_actor.apply_poll_results(poll_iter).await;

        // Record timestamps for state-changed hosts (for enforce stabilization timer).
        // We compare the current watch snapshot with the pre-poll snapshot.
        {
            let current_status = state.host_actor.snapshot();
            for (host, new_state) in current_status.iter() {
                if pre_poll_status.get(host) != Some(new_state) {
                    state_timestamps.insert(host.clone(), poll_start);
                }
            }
        }

        // Enforce state for hosts that opt in, after a stabilization delay.
        let leases_snapshot = state.leases.snapshot();
        for (host_name, host_cfg) in &config.hosts {
            let lease_set = leases_snapshot.get(host_name).cloned().unwrap_or_default();
            let current_state = state.host_actor.get_current_state(host_name);

            let stable_for = state_timestamps
                .get(host_name)
                .map_or(enforce_threshold, Instant::elapsed);

            if should_enforce_action(
                host_cfg,
                &lease_set,
                current_state,
                stable_for,
                enforce_threshold,
            ) {
                spawn_handle_host_state(host_name, &state);
            }
        }

        ticker.tick().await;
    }
}

/// Fetches permanent online-for subscriptions for `hostname` and spawns a deferred
/// task for each one. Each task sleeps for the subscribed duration, then checks that
/// the host is still in the same online session (via `online_since`) before sending
/// the push notification.
async fn spawn_online_for_notifications(
    hostname: &str,
    session_start: Instant,
    online_since: &Arc<RwLock<HashMap<String, Instant>>>,
    pool: &db::DbPool,
    vapid_key: &Arc<ES256KeyPair>,
) {
    match db::get_subscriptions_for_host_online_for(pool, hostname).await {
        Ok(subs) if !subs.is_empty() => {
            for (sub, duration_secs) in subs {
                let duration = Duration::from_secs(u64::try_from(duration_secs).unwrap_or(0));
                let hostname = hostname.to_string();
                let online_since = online_since.clone();
                let vapid_key = vapid_key.clone();
                let pool = pool.clone();
                tokio::spawn(async move {
                    sleep(duration).await;
                    // Only fire if the host is still in the same online session.
                    if online_since.read().await.get(&hostname) != Some(&session_start) {
                        return;
                    }
                    let payload = push::NotificationPayload::with_data(
                        format!("{hostname} has been online for {duration_secs} seconds"),
                        push::HostSpecificNotificationData { hostname },
                    )
                    .into_json();
                    push::send_push_notifications(&vapid_key, &pool, &[sub], &payload).await;
                });
            }
        }
        Ok(_) => {}
        Err(e) => {
            error!(host = %hostname, "Failed to fetch online-for push subscriptions: {e:#}");
        }
    }
}

/// Background task: consumes the [`HostEvent`] stream and fires push notifications
/// for unscheduled host state transitions.
///
/// An event is "unscheduled" when:
/// - `Offline → Online` with no active leases (host booted without coordinator involvement).
/// - `Online → Offline` while leases are held (host went offline unexpectedly).
async fn handle_host_events(
    mut events_rx: broadcast::Receiver<HostEvent>,
    initial_leases: Arc<LeaseMapRaw>,
    db_pool: Option<db::DbPool>,
    vapid_key: Option<Arc<ES256KeyPair>>,
    mut leases_rx: LeaseRx,
) {
    // Keep a local copy of the lease map so we can check it at event time without
    // holding a lock on the LeaseStore.
    let mut current_leases = initial_leases;

    loop {
        tokio::select! {
            // Update our local lease snapshot whenever it changes.
            result = leases_rx.changed() => {
                if result.is_err() {
                    break;
                }
                current_leases = leases_rx.borrow().clone();
            }
            // React to host state transitions.
            event = events_rx.recv() => {
                let event = match event {
                    Ok(e) => e,
                    Err(RecvError::Lagged(n)) => {
                        warn!("handle_host_events: missed {n} events (broadcast channel lagged)");
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };

                let HostEvent::StateChanged { host: host_name, from, to, .. } = event else {
                    continue;
                };

                let has_leases = current_leases
                    .get(&host_name)
                    .is_some_and(|s| !s.is_empty());

                let is_unscheduled = match (from, to) {
                    // Host came online without the coordinator waking it.
                    (HostState::Offline, HostState::Online) => !has_leases,
                    // Host went offline while the coordinator expected it to stay up.
                    (HostState::Online, HostState::Offline) => has_leases,
                    _ => false,
                };

                if !is_unscheduled {
                    continue;
                }

                let body = match to {
                    HostState::Online => format!("{host_name} started up unexpectedly"),
                    HostState::Offline => format!("{host_name} shut down unexpectedly"),
                    HostState::Waking | HostState::ShuttingDown => continue,
                };

                let (Some(pool), Some(vapid_key)) = (db_pool.clone(), vapid_key.clone()) else {
                    continue;
                };

                tokio::spawn(async move {
                    match db::get_subscriptions_for_host_unscheduled(&pool, &host_name).await {
                        Ok(subs) if !subs.is_empty() => {
                            let payload = push::NotificationPayload::with_data(
                                body,
                                push::HostSpecificNotificationData {
                                    hostname: host_name,
                                },
                            )
                            .into_json();
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
    }
}

/// Background task: watches the lease store and forwards per-host lease changes
/// into the [`HostActorHandle`] event stream so all consumers can use a single stream.
async fn forward_lease_events(mut leases_rx: LeaseRx, host_actor: HostActorHandle) {
    let mut prev_leases: Arc<LeaseMapRaw> = leases_rx.borrow_and_update().clone();
    while leases_rx.changed().await.is_ok() {
        let new_leases: Arc<LeaseMapRaw> = leases_rx.borrow_and_update().clone();
        // Notify the actor for each host whose lease set changed.
        let all_hosts: HashSet<&str> = prev_leases
            .keys()
            .chain(new_leases.keys())
            .map(String::as_str)
            .collect();
        for host in all_hosts {
            if prev_leases.get(host) != new_leases.get(host) {
                let leases = new_leases.get(host).cloned().unwrap_or_default();
                host_actor
                    .notify_lease_changed(host.to_string(), leases)
                    .await;
            }
        }
        prev_leases = new_leases;
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
        let hoststatus = state.host_actor.borrow();

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

    state.host_actor.startup_broadcast(hostname).await;

    maybe_update_host_install_info(
        state,
        hostname,
        startup.agent_version.clone(),
        startup.init_system,
        startup.os,
        None,
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
            Duration::ZERO,
            ENFORCE_STABILIZATION_THRESHOLD,
        ));

        let cfg = make_host(true);
        // no mismatch: both offline
        assert!(!should_enforce_action(
            &cfg,
            &lease_set,
            HostState::Offline,
            Duration::from_secs(100),
            ENFORCE_STABILIZATION_THRESHOLD,
        ));
        // mismatch but short stable time
        let lease_set: LeaseSources = vec![LeaseSource::WebInterface].into_iter().collect();
        assert!(!should_enforce_action(
            &cfg,
            &lease_set,
            HostState::Offline,
            Duration::from_secs(1),
            ENFORCE_STABILIZATION_THRESHOLD,
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
                .unwrap(),
            ENFORCE_STABILIZATION_THRESHOLD,
        ));
        assert!(should_enforce_action(
            &cfg,
            &lease_set,
            current,
            ENFORCE_STABILIZATION_THRESHOLD,
            ENFORCE_STABILIZATION_THRESHOLD,
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
                .map(|i| (i.agent_version, i.init_system, i.os, i.script_path)),
            Some((
                Some("v1.2.3".to_string()),
                Some(InitSystem::Systemd),
                Some(OsType::Linux),
                None,
            ))
        );
        assert_eq!(
            parse_install_info("OK: status;agent_version=v1.2.3; script_path=/tmp/foo.sh")
                .map(|i| (i.agent_version, i.script_path)),
            Some((Some("v1.2.3".to_string()), Some("/tmp/foo.sh".to_string())))
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
