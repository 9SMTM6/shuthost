//! HMAC validation for the unified frontend-API M2M path.
//!
//! # Stability Warning
//!
//! This module exposes `/api/*` (the frontend UI backend) to M2M clients as
//! a **fallback only**.  Unlike the stable `/api/m2m/*` endpoints, endpoints
//! reached through this path carry **no stability guarantees**:
//!
//! - Endpoints, response shapes, and semantics change without notice.
//! - The `EXPECTED_FRONTEND_ENDPOINT_VERSION` constant is a kill switch, not
//!   a version negotiation mechanism.  When bumped, all outdated clients are
//!   rejected with a hard 403 — there is no grace period or co-existence.
//! - Only fire-and-forget semantics are supported (no `?async`, sync-wait,
//!   or per-host status).
//!
//! Production automation should use the stable `/api/m2m/*` endpoints instead.
//!
//! # Header Format
//!
//! Reuses the same `X-Client-ID` / `X-Request` header format as `/api/m2m/*`:
//!
//! ```text
//! X-Client-ID: <client_id>
//! X-Request:   <unix_timestamp>|<version_string>|<hex_hmac_sha256>
//! ```
//!
//! The `version_string` must match `EXPECTED_FRONTEND_ENDPOINT_VERSION`
//! exactly.  Unlike the stable M2M path, no action matching is performed —
//! the signed message is compared verbatim to the expected version string.

use alloc::sync::Arc;

use axum::http::{HeaderMap, StatusCode};
use shuthost_common::validate_hmac_message;
use tracing::{info, warn};

use crate::config::ControllerConfig;

/// The expected frontend-endpoint version string.
///
/// Clients embed this exact string as the "message" portion of their
/// `X-Request` header (e.g. `timestamp|frontendpointV1|hex_sig`).
///
/// Bump this to a new value (e.g. `frontendpointV2`) when making breaking
/// changes to the frontend API. Outdated clients will receive a 403 response.
pub(crate) const EXPECTED_FRONTEND_ENDPOINT_VERSION: &str = "frontendpointV1";

/// Paths where M2M authentication is rejected even with valid credentials.
pub(crate) const M2M_BLOCKED_PREFIXES: &[&str] = &[
    "/api/push/",
    "/ws",
];

/// Validates HMAC identity headers and returns the verified `client_id`.
///
/// On success the returned `String` is the verified client identifier.
/// On failure a `(StatusCode, &'static str)` error tuple is returned.
pub(crate) fn validate_hmac_identity(
    headers: &HeaderMap,
    config: &Arc<ControllerConfig>,
) -> Result<String, (StatusCode, &'static str)> {
    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Client-ID"))?;

    let data_str = headers
        .get("X-Request")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Request"))?;

    let shared_secret = config
        .clients
        .get(client_id)
        .ok_or_else(|| {
            warn!("Unknown client '{}'", client_id);
            (StatusCode::FORBIDDEN, "Unknown client")
        })?
        .shared_secret
        .clone();

    let version = match validate_hmac_message(data_str, shared_secret.as_ref()) {
        shuthost_common::HmacValidationResult::Valid(msg) => msg,
        shuthost_common::HmacValidationResult::InvalidTimestamp => {
            info!("Timestamp out of range for client '{}'", client_id);
            return Err((StatusCode::UNAUTHORIZED, "Timestamp out of range"));
        }
        shuthost_common::HmacValidationResult::InvalidHmac => {
            info!("Invalid HMAC signature for client '{}'", client_id);
            return Err((StatusCode::UNAUTHORIZED, "Invalid HMAC signature"));
        }
        shuthost_common::HmacValidationResult::MalformedMessage => {
            return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
        }
    };

    if version != EXPECTED_FRONTEND_ENDPOINT_VERSION {
        info!(
            "Client '{}' used outdated frontend-endpoint version '{}', expected '{}'",
            client_id, version, EXPECTED_FRONTEND_ENDPOINT_VERSION
        );
        return Err((StatusCode::FORBIDDEN, "Outdated client version"));
    }

    Ok(client_id.to_string())
}
