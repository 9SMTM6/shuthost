mod config_watcher;
pub mod db;
mod runtime;
mod startup;
mod state;

// Re-export a curated crate-visible surface for consumers of `crate::app`
pub(crate) use db::DbPool;
pub(crate) use runtime::poll_until_host_state;
pub(crate) use startup::{shutdown_signal, start};
pub(crate) use state::{AppState, ConfigRx, HostStatus, HostStatusRx, HostStatusTx, WsTx};
