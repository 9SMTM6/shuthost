//! HMAC validation and request parsing for M2M endpoints.
//!
//! The cryptographic core lives in [`crate::http::auth::hmac`]; this module
//! only adds the stable-API semantics: the signed message is parsed as an
//! action and must match the requested operation.

use axum::http::{HeaderMap, StatusCode};

use crate::{app::AppState, http::api::LeaseAction, http::auth::hmac};

/// Validates M2M lease action request headers and returns (`client_id`, `LeaseAction`)
pub(crate) fn validate_m2m_request(
    headers: &HeaderMap,
    state: &AppState,
    expected_action: LeaseAction,
) -> Result<String, (StatusCode, &'static str)> {
    let (client_id, command) = hmac::validate_hmac_headers(headers, &state.config_rx.borrow())?;

    let command_action: LeaseAction = serde_plain::from_str(&command)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid action in X-Request"))?;

    if command_action != expected_action {
        return Err((StatusCode::BAD_REQUEST, "Action mismatch"));
    }

    Ok(client_id)
}

/// Validates M2M status request headers and returns `client_id`.
pub(crate) fn validate_m2m_status_request(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<String, (StatusCode, &'static str)> {
    let (client_id, command) = hmac::validate_hmac_headers(headers, &state.config_rx.borrow())?;

    if command != "status" {
        return Err((StatusCode::BAD_REQUEST, "Action mismatch"));
    }

    Ok(client_id)
}
