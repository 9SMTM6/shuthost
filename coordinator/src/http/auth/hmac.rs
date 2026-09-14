//! HMAC validation for M2M requests.
//!
//! The [`validate_hmac_headers`] core is shared by both HMAC-authenticated
//! paths:
//!
//! - the stable `/api/m2m/*` endpoints ([`crate::http::m2m`]), which parse
//!   the signed message as an action and require it to match the requested
//!   operation;
//! - the frontend `/api/*` fallback path ([`validate_hmac_identity`], used by
//!   the auth middleware), which requires the signed message to equal
//!   [`EXPECTED_FRONTEND_ENDPOINT_VERSION`] exactly.
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
//! Both paths reuse the same `X-Client-ID` / `X-Request` header format:
//!
//! ```text
//! X-Client-ID: <client_id>
//! X-Request:   <unix_timestamp>|<message>|<hex_hmac_sha256>
//! ```
//!
//! The `message` is a per-action string for the stable M2M endpoints, and
//! must match `EXPECTED_FRONTEND_ENDPOINT_VERSION` exactly for the frontend
//! fallback path.

use axum::http::{HeaderMap, StatusCode};
use shuthost_common::{HmacValidationResult, validate_hmac_message};
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
pub(crate) const M2M_BLOCKED_PREFIXES: &[&str] = &["/api/push/", "/ws"];

/// Validates the `X-Client-ID` / `X-Request` HMAC identity headers and
/// returns the verified `client_id` together with the signed message.
///
/// Callers decide how to interpret the returned message: the stable
/// `/api/m2m/*` endpoints parse it as an action and require it to match the
/// requested operation, while the frontend fallback path requires it to equal
/// [`EXPECTED_FRONTEND_ENDPOINT_VERSION`] (see [`validate_hmac_identity`]).
///
/// On failure a `(StatusCode, &'static str)` error tuple is returned.
pub(crate) fn validate_hmac_headers(
    headers: &HeaderMap,
    config: &ControllerConfig,
) -> Result<(String, String), (StatusCode, &'static str)> {
    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Client-ID"))?;

    let data_str = headers
        .get("X-Request")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Request"))?;

    // potential enumeration issue, if thats something we want to cover.
    let shared_secret = config
        .clients
        .get(client_id)
        .ok_or_else(|| {
            warn!("Unknown client '{client_id}'");
            (StatusCode::FORBIDDEN, "Unknown client")
        })?
        .shared_secret
        .clone();

    use HmacValidationResult as HVR;

    let message = match validate_hmac_message(data_str, shared_secret.as_ref()) {
        HVR::Valid(msg) => msg,
        HVR::InvalidTimestamp => {
            info!("Timestamp out of range for client '{client_id}'");
            return Err((StatusCode::UNAUTHORIZED, "Timestamp out of range"));
        }
        HVR::InvalidHmac => {
            info!("Invalid HMAC signature for client '{client_id}'");
            return Err((StatusCode::UNAUTHORIZED, "Invalid HMAC signature"));
        }
        HVR::MalformedMessage => {
            return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
        }
    };

    Ok((client_id.to_string(), message))
}

/// Validates HMAC identity headers for the frontend `/api/*` fallback path
/// and returns the verified `client_id`.
///
/// The signed message must equal [`EXPECTED_FRONTEND_ENDPOINT_VERSION`]
/// exactly; clients presenting an outdated version are rejected with a 403.
pub(crate) fn validate_hmac_identity(
    headers: &HeaderMap,
    config: &ControllerConfig,
) -> Result<String, (StatusCode, &'static str)> {
    let (client_id, version) = validate_hmac_headers(headers, config)?;

    if version != EXPECTED_FRONTEND_ENDPOINT_VERSION {
        info!(
            "Client '{client_id}' used outdated frontend-endpoint version '{version}', expected '{EXPECTED_FRONTEND_ENDPOINT_VERSION}'"
        );
        return Err((StatusCode::FORBIDDEN, "Outdated client version"));
    }

    Ok(client_id)
}
