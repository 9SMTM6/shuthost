mod config_watcher;
pub mod db;
mod host_control;
mod runtime;
mod startup;
mod state;

// Re-export a curated crate-visible surface for consumers of `crate::app`
pub(crate) use db::DbPool;
pub(crate) use host_control::{
    HostControlError, LeaseMapRaw, LeaseRx, LeaseSource, LeaseSources, LeaseState,
    handle_host_state,
};
pub(crate) use startup::{shutdown_signal, start};
pub(crate) use state::{AppState, ConfigRx, HostStatusRx, WsTx};
pub use runtime::ENFORCE_STABILIZATION_THRESHOLD;

pub use state::{HostState, HostStatus};
