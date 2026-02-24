//! HTTP server implementation for the coordinator control interface.
//!
//! Defines routes, state management, configuration watching, and server startup.
//!
//! This file contains the public re-exports and shared constants/macros for the
//! split `server` submodules (`state`, `tls`, `router`, `middleware`, `run`).

/// Version number for validating external authentication exceptions.
///
/// This constant ensures compatibility with external authentication systems by checking
/// the exceptions version against expected values. It is used in authentication resolution
/// logic to validate external auth modes.
///
/// It is interdependent with the [`create_app_router`] function in this module, as the public routes
/// defined there include authentication endpoints (e.g., login, logout, OIDC callbacks) whose behavior and
/// accessibility may depend on this version when handling external authentication modes.
/// When routes get added to public routes, this needs to be bumped.
pub(crate) const EXPECTED_AUTH_EXCEPTIONS_VERSION: u32 = 2;

#[macro_export]
macro_rules! cfg_if_expr {
    (
        #[cfg($condition: meta)]
        $true_block: expr,
        #[cfg(not)]
        $false_block: expr,
    ) => {{
        #[cfg($condition)]
        let _return = $true_block;
        #[cfg(not($condition))]
        let _return = $false_block;
        _return
    }};
}

pub mod middleware;
pub mod router;
pub mod tls;

#[expect(clippy::missing_const_for_fn, reason = "used as compilation test")]
fn _test_cfg_if_expr() {
    let _var = cfg_if_expr!(
        #[cfg(target_arch = "wasm32")]
        {
            4
        },
        #[cfg(not)]
        {
            5
        },
    );
}
