mod config_watcher;
pub mod db;
pub(crate) mod host_actor;
mod host_control;
pub(crate) mod notifications;
mod runtime;
mod shared_watch_store;
mod startup;
mod state;

// Re-export a curated crate-visible surface for consumers of `crate::app`
pub(crate) use db::DbPool;
pub(crate) use host_actor::HostActorHandle;
pub(crate) use host_control::{
    HostControlError, LeaseMapRaw, LeaseRx, LeaseSource, LeaseSources, LeaseStore, lookup_host,
    lookup_host_with_overrides, wait_for_transition,
};
pub use runtime::ENFORCE_STABILIZATION_THRESHOLD;
pub(crate) use startup::{shutdown_signal, start};
pub(crate) use state::{AppState, ConfigRx, HostStatusRx, RwMap, WsTx};

pub(crate) use state::OperationFailureStore;
pub use state::{HostState, HostStatus, OperationFailure, OperationFailureMap, OperationKind};
