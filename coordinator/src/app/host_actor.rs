//! Single-owner host state machine.
//!
//! The [`HostActor`] task is the sole writer of host states. All other components
//! (background poller, startup broadcast listener, control tasks) send [`HostCmd`]
//! messages to the actor instead of writing shared state directly.
//!
//! This eliminates the race that caused visible `Online→Offline` flicker during
//! waking: previously a startup broadcast would exit the `Waking` guard, allowing
//! the next poller cycle to write `Offline` before the control task finished.
//! Now the actor tracks a separate `control_active` set so that poll results are
//! ignored for any host whose control task is still in-flight, regardless of the
//! visible state.

use alloc::sync::Arc;
use std::collections::HashSet;

use tokio::{
    sync::{broadcast, mpsc, oneshot, watch},
    time::Instant,
};
use tracing::{debug, warn};

use crate::app::{
    host_control::LeaseSources,
    state::{HostState, HostStatus, OperationKind},
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The result of a completed host control operation.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TransitionResult {
    /// Wake (startup) succeeded – host is now Online.
    WakeOk,
    /// Wake (startup) failed – host should be treated as Offline.
    WakeErr,
    /// Shutdown succeeded – host is now Offline.
    ShutdownOk,
    /// Shutdown failed – host should be treated as Online.
    ShutdownErr,
}

/// An event emitted by the actor whenever host state or lease membership changes.
///
/// Subscribers can use this stream to react to transitions in a single, ordered
/// place rather than watching multiple independent channels.
#[expect(dead_code)]
#[derive(Debug, Clone)]
pub(crate) enum HostEvent {
    /// A host's visible [`HostState`] changed.
    StateChanged {
        host: String,
        from: HostState,
        to: HostState,
        /// Wall-clock instant of the change (from the actor's perspective).
        at: Instant,
    },
    /// The lease set for a host changed.
    LeaseChanged {
        host: String,
        leases: LeaseSources,
        at: Instant,
    },
}

// ---------------------------------------------------------------------------
// Internal command type
// ---------------------------------------------------------------------------

pub(crate) enum HostCmd {
    /// Batch of polled (Online/Offline) observations from the background poller.
    /// Ignored for any host currently under control-task ownership.
    /// The actor sends the resulting snapshot back via `reply` once processed.
    PollResults {
        results: Vec<(String, HostState)>,
        reply: oneshot::Sender<Arc<HostStatus>>,
    },

    /// A valid startup broadcast was received from a host agent.
    /// Sets the host Online (even during `Waking`) but does NOT release
    /// control-task ownership – the control task must still call
    /// [`HostCmd::TransitionComplete`] to release the lock.
    StartupBroadcast { host: String },

    /// Atomically claim a transition slot for `host`.
    ///
    /// Replies `true` if the slot was claimed (host is now `Waking` or
    /// `ShuttingDown` and added to `control_active`), `false` if already
    /// claimed or already in a transition state.
    BeginTransition {
        host: String,
        direction: OperationKind,
        reply: oneshot::Sender<bool>,
    },

    /// The control task finished for `host`. Releases ownership and writes the
    /// final state derived from `result`.
    TransitionComplete {
        host: String,
        result: TransitionResult,
    },

    /// The lease set for `host` changed; the actor re-emits it as a
    /// [`HostEvent::LeaseChanged`] so all consumers can use a single stream.
    LeaseChanged { host: String, leases: LeaseSources },
}

// ---------------------------------------------------------------------------
// Actor
// ---------------------------------------------------------------------------

struct HostActor {
    /// Visible state of each known host.
    states: HostStatus,
    /// Hosts whose state is owned by an in-flight control task.
    /// Poll results are ignored for these hosts.
    control_active: HashSet<String>,
    /// Watch channel – published to on every state change.
    status_tx: Arc<watch::Sender<Arc<HostStatus>>>,
    /// Broadcast channel – events emitted on state & lease changes.
    event_tx: Arc<broadcast::Sender<HostEvent>>,
}

impl HostActor {
    /// Apply a state transition for `host`, publishing to the watch and event
    /// channels if the state actually changed.
    fn apply_state_change(&mut self, host: &str, new_state: HostState) {
        let old = self.states.get(host).copied().unwrap_or(HostState::Offline);
        if old == new_state {
            return;
        }
        self.states.insert(host.to_string(), new_state);

        // Publish full snapshot to the watch channel.
        // Use send_replace so the stored value is always updated, even in
        // tests where the initial receiver has been dropped.
        let mut snapshot = self.status_tx.borrow().as_ref().clone();
        snapshot.insert(host.to_string(), new_state);
        drop(self.status_tx.send_replace(Arc::new(snapshot)));

        // Emit a typed event.
        drop(self.event_tx.send(HostEvent::StateChanged {
            host: host.to_string(),
            from: old,
            to: new_state,
            at: Instant::now(),
        }));
    }

    fn handle_cmd(&mut self, cmd: HostCmd) {
        match cmd {
            HostCmd::PollResults { results, reply } => {
                for (host, new_state) in results {
                    // Ignore if a control task is in-flight for this host.
                    if self.control_active.contains(&host) {
                        continue;
                    }
                    let current = self
                        .states
                        .get(&host)
                        .copied()
                        .unwrap_or(HostState::Offline);
                    // Safety belt: don't let poll results overwrite a transitioning
                    // state that was set without control_active (shouldn't happen, but
                    // be defensive).
                    if current.is_transitioning() {
                        warn!(
                            host = %host,
                            "Ignoring poll result {:?} for host in transitioning state {:?} \
                             without control_active (unexpected)",
                            new_state, current,
                        );
                        continue;
                    }
                    self.apply_state_change(&host, new_state);
                }
                // Reply with the post-apply snapshot so the caller observes the
                // definitive state without a separate watch read.
                drop(reply.send(self.status_tx.borrow().clone()));
            }

            HostCmd::StartupBroadcast { host } => {
                // The control task (if any) remains the owner; we just update
                // the visible state so the UI and watch subscribers are current.
                self.apply_state_change(&host, HostState::Online);
            }

            HostCmd::BeginTransition {
                host,
                direction,
                reply,
            } => {
                let current = self
                    .states
                    .get(&host)
                    .copied()
                    .unwrap_or(HostState::Offline);
                if self.control_active.contains(&host) || current.is_transitioning() {
                    debug!(
                        host = %host,
                        "BeginTransition rejected: control_active={}, state={:?}",
                        self.control_active.contains(&host),
                        current,
                    );
                    let _ = reply.send(false);
                    return;
                }
                let transition_state = match direction {
                    OperationKind::Startup => HostState::Waking,
                    OperationKind::Shutdown => HostState::ShuttingDown,
                };
                self.control_active.insert(host.clone());
                self.apply_state_change(&host, transition_state);
                let _ = reply.send(true);
            }

            HostCmd::TransitionComplete { host, result } => {
                self.control_active.remove(&host);
                let final_state = match result {
                    TransitionResult::WakeOk | TransitionResult::ShutdownErr => HostState::Online,
                    TransitionResult::WakeErr | TransitionResult::ShutdownOk => HostState::Offline,
                };
                self.apply_state_change(&host, final_state);
            }

            HostCmd::LeaseChanged { host, leases } => {
                drop(self.event_tx.send(HostEvent::LeaseChanged {
                    host,
                    leases,
                    at: Instant::now(),
                }));
            }
        }
    }

    async fn run(mut self, mut rx: mpsc::Receiver<HostCmd>) {
        while let Some(cmd) = rx.recv().await {
            self.handle_cmd(cmd);
        }
    }
}

// ---------------------------------------------------------------------------
// Handle (public interface)
// ---------------------------------------------------------------------------

/// A cheaply-cloneable handle to the [`HostActor`] task.
///
/// All write operations send commands to the actor (async, non-blocking once
/// the channel has capacity). Read operations go directly through the
/// `watch::Sender` without touching the actor task.
#[derive(Clone)]
pub(crate) struct HostActorHandle {
    tx: mpsc::Sender<HostCmd>,
    /// Held so callers can call `.subscribe()` / `.borrow()` / `.send_if_modified()`.
    pub(crate) status_tx: Arc<watch::Sender<Arc<HostStatus>>>,
    /// Held so callers can call `.subscribe()` to receive events.
    event_tx: Arc<broadcast::Sender<HostEvent>>,
}

impl HostActorHandle {
    /// Spawn the actor task and return the handle.
    pub(crate) fn spawn(initial: HostStatus) -> Self {
        let (status_tx, _) = watch::channel(Arc::new(initial.clone()));
        let status_tx = Arc::new(status_tx);
        let (event_tx, _) = broadcast::channel(256);
        let event_tx = Arc::new(event_tx);
        let (cmd_tx, cmd_rx) = mpsc::channel(256);

        let actor = HostActor {
            states: initial,
            control_active: HashSet::new(),
            status_tx: Arc::clone(&status_tx),
            event_tx: Arc::clone(&event_tx),
        };
        tokio::spawn(actor.run(cmd_rx));

        Self {
            tx: cmd_tx,
            status_tx,
            event_tx,
        }
    }

    // ------------------------------------------------------------------
    // Write operations (send to actor)
    // ------------------------------------------------------------------

    /// Atomically begin a host state transition.
    ///
    /// Returns `true` if the transition was claimed (`Waking`/`ShuttingDown`
    /// has been set). Returns `false` if the host is already under control or
    /// already transitioning.
    pub(crate) async fn begin_transition(&self, host: &str, direction: OperationKind) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = HostCmd::BeginTransition {
            host: host.to_string(),
            direction,
            reply: reply_tx,
        };
        // If the actor has shut down, treat as "could not claim" (false).
        if self.tx.send(cmd).await.is_err() {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    /// Signal that the control task for `host` has completed with `result`.
    pub(crate) async fn transition_complete(&self, host: &str, result: TransitionResult) {
        let cmd = HostCmd::TransitionComplete {
            host: host.to_string(),
            result,
        };
        drop(self.tx.send(cmd).await);
    }

    /// Submit a batch of polled (Online/Offline) observations.
    ///
    /// Hosts with an active control task are silently skipped by the actor.
    /// Returns the post-apply host-status snapshot (via a oneshot reply from
    /// the actor), so the caller sees the definitive state synchronously.
    pub(crate) async fn apply_poll_results(
        &self,
        results: impl IntoIterator<Item = (String, HostState)>,
    ) -> Arc<HostStatus> {
        let vec: Vec<_> = results.into_iter().collect();
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(HostCmd::PollResults {
                results: vec,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            // Actor has shut down; fall back to the current watch value.
            return self.snapshot();
        }
        reply_rx.await.unwrap_or_else(|_| self.snapshot())
    }

    /// Signal that a valid startup broadcast was received for `host`.
    pub(crate) async fn startup_broadcast(&self, host: &str) {
        drop(
            self.tx
                .send(HostCmd::StartupBroadcast {
                    host: host.to_string(),
                })
                .await,
        );
    }

    /// Notify the actor (and event stream subscribers) that the lease set for
    /// `host` changed.
    pub(crate) async fn notify_lease_changed(&self, host: String, leases: LeaseSources) {
        drop(self.tx.send(HostCmd::LeaseChanged { host, leases }).await);
    }

    // ------------------------------------------------------------------
    // Read operations (direct, no actor round-trip)
    // ------------------------------------------------------------------

    /// Return the current state of `host`, defaulting to `Offline` if unknown.
    pub(crate) fn get_current_state(&self, host: &str) -> HostState {
        self.status_tx
            .borrow()
            .get(host)
            .copied()
            .unwrap_or(HostState::Offline)
    }

    /// Return a cloned snapshot of the current host-status map.
    pub(crate) fn snapshot(&self) -> Arc<HostStatus> {
        self.status_tx.borrow().clone()
    }

    /// Borrow a reference to the current host-status snapshot (cheap, no clone).
    pub(crate) fn borrow(&self) -> watch::Ref<'_, Arc<HostStatus>> {
        self.status_tx.borrow()
    }

    /// Subscribe to future host-status snapshots.
    pub(crate) fn subscribe_status(&self) -> watch::Receiver<Arc<HostStatus>> {
        self.status_tx.subscribe()
    }

    /// Subscribe to the typed [`HostEvent`] stream.
    pub(crate) fn subscribe_events(&self) -> broadcast::Receiver<HostEvent> {
        self.event_tx.subscribe()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::host_control::LeaseSource;
    use std::collections::{HashMap, HashSet};

    // Helper: build a minimal actor (not spawned, runs in-process via handle_cmd)
    fn make_actor() -> HostActor {
        let (status_tx, _) = watch::channel(Arc::new(HostStatus::new()));
        let (event_tx, _) = broadcast::channel(64);
        HostActor {
            states: HashMap::new(),
            control_active: HashSet::new(),
            status_tx: Arc::new(status_tx),
            event_tx: Arc::new(event_tx),
        }
    }

    // -------------------------------------------------------------------
    // Transition table correctness
    // -------------------------------------------------------------------

    #[test]
    fn begin_transition_sets_waking_and_control_active() {
        let mut actor = make_actor();
        let (tx, mut rx) = oneshot::channel();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx,
        });
        assert!(rx.try_recv().unwrap());
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Waking,);
        assert!(actor.control_active.contains("srv"));
        assert_eq!(
            actor.status_tx.borrow().get("srv").copied(),
            Some(HostState::Waking)
        );
    }

    #[test]
    fn begin_transition_rejected_if_already_active() {
        let mut actor = make_actor();
        // First claim
        let (tx1, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx1,
        });
        // Second claim while active
        let (tx2, mut rx2) = oneshot::channel();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx2,
        });
        assert!(!rx2.try_recv().unwrap());
        // State unchanged
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Waking);
    }

    #[test]
    fn transition_complete_wake_ok_sets_online_and_clears_control() {
        let mut actor = make_actor();
        let (tx, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx,
        });
        actor.handle_cmd(HostCmd::TransitionComplete {
            host: "srv".to_string(),
            result: TransitionResult::WakeOk,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Online);
        assert!(!actor.control_active.contains("srv"));
    }

    #[test]
    fn transition_complete_wake_err_sets_offline_and_clears_control() {
        let mut actor = make_actor();
        let (tx, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx,
        });
        actor.handle_cmd(HostCmd::TransitionComplete {
            host: "srv".to_string(),
            result: TransitionResult::WakeErr,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Offline);
        assert!(!actor.control_active.contains("srv"));
    }

    #[test]
    fn transition_complete_shutdown_ok_sets_offline() {
        let mut actor = make_actor();
        // Simulate host already online so ShuttingDown is a valid next state
        actor.states.insert("srv".to_string(), HostState::Online);
        let (tx, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Shutdown,
            reply: tx,
        });
        actor.handle_cmd(HostCmd::TransitionComplete {
            host: "srv".to_string(),
            result: TransitionResult::ShutdownOk,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Offline);
    }

    #[test]
    fn transition_complete_shutdown_err_sets_online() {
        let mut actor = make_actor();
        actor.states.insert("srv".to_string(), HostState::Online);
        let (tx, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Shutdown,
            reply: tx,
        });
        actor.handle_cmd(HostCmd::TransitionComplete {
            host: "srv".to_string(),
            result: TransitionResult::ShutdownErr,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Online);
    }

    // -------------------------------------------------------------------
    // Flicker fix: the core regression test
    // -------------------------------------------------------------------

    /// Verifies that a `StartupBroadcast` during Waking does NOT allow a subsequent
    /// `PollResults` to set the host Offline (the control-active guard must hold).
    #[test]
    fn poll_results_ignored_while_control_active_even_after_startup_broadcast() {
        let mut actor = make_actor();

        // Step 1: control task claims Waking
        let (tx, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Waking);

        // Step 2: startup broadcast arrives → visible state becomes Online
        actor.handle_cmd(HostCmd::StartupBroadcast {
            host: "srv".to_string(),
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Online);
        // control_active still set
        assert!(actor.control_active.contains("srv"));

        // Step 3: poller fires and observes Offline (early poll during boot) → must be IGNORED
        let (reply_tx, _reply_rx) = oneshot::channel();
        actor.handle_cmd(HostCmd::PollResults {
            results: vec![("srv".to_string(), HostState::Offline)],
            reply: reply_tx,
        });
        // State must remain Online, not flicker to Offline
        assert_eq!(
            *actor.states.get("srv").unwrap(),
            HostState::Online,
            "State must not flicker to Offline while control task is active"
        );

        // Step 4: control task completes successfully → Online confirmed
        actor.handle_cmd(HostCmd::TransitionComplete {
            host: "srv".to_string(),
            result: TransitionResult::WakeOk,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Online);
        assert!(!actor.control_active.contains("srv"));

        // Step 5: now poller can write (control_active cleared)
        let (reply_tx, _reply_rx) = oneshot::channel();
        actor.handle_cmd(HostCmd::PollResults {
            results: vec![("srv".to_string(), HostState::Offline)],
            reply: reply_tx,
        });
        assert_eq!(*actor.states.get("srv").unwrap(), HostState::Offline);
    }

    // -------------------------------------------------------------------
    // Event emission
    // -------------------------------------------------------------------

    #[test]
    fn state_change_emits_event() {
        let mut actor = make_actor();
        let mut ev_rx = actor.event_tx.subscribe();

        let (tx, _) = oneshot::channel::<bool>();
        actor.handle_cmd(HostCmd::BeginTransition {
            host: "srv".to_string(),
            direction: OperationKind::Startup,
            reply: tx,
        });

        let ev = ev_rx.try_recv().expect("event should be available");
        match ev {
            HostEvent::StateChanged { host, from, to, .. } => {
                assert_eq!(host, "srv");
                assert_eq!(from, HostState::Offline);
                assert_eq!(to, HostState::Waking);
            }
            HostEvent::LeaseChanged { .. } => panic!("unexpected event"),
        }
    }

    #[test]
    fn lease_changed_emits_event() {
        let mut actor = make_actor();
        let mut ev_rx = actor.event_tx.subscribe();

        let leases: LeaseSources = vec![LeaseSource::WebInterface].into_iter().collect();
        actor.handle_cmd(HostCmd::LeaseChanged {
            host: "srv".to_string(),
            leases: leases.clone(),
        });

        let ev = ev_rx.try_recv().expect("event should be available");
        match ev {
            HostEvent::LeaseChanged {
                host,
                leases: got_leases,
                ..
            } => {
                assert_eq!(host, "srv");
                assert_eq!(got_leases, leases);
            }
            HostEvent::StateChanged { .. } => panic!("unexpected event"),
        }
    }

    #[test]
    fn no_duplicate_event_when_state_unchanged() {
        let mut actor = make_actor();
        actor.states.insert("srv".to_string(), HostState::Online);
        let mut ev_rx = actor.event_tx.subscribe();

        // Poll result for same state → no event
        let (reply_tx, _reply_rx) = oneshot::channel();
        actor.handle_cmd(HostCmd::PollResults {
            results: vec![("srv".to_string(), HostState::Online)],
            reply: reply_tx,
        });
        assert!(
            ev_rx.try_recv().is_err(),
            "no event expected for no-op state write"
        );
    }
}
