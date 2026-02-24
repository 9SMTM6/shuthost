//! HMAC validation and request parsing for M2M endpoints.

use axum::http::{HeaderMap, StatusCode};
use shuthost_common::validate_hmac_message;
use tracing::{info, warn};

use crate::{http::api::LeaseAction, state::AppState};

/// Validates M2M lease action request headers and returns (`client_id`, `LeaseAction`)
pub(crate) fn validate_m2m_request(
    headers: &HeaderMap,
    state: &AppState,
    expected_action: LeaseAction,
) -> Result<String, (StatusCode, &'static str)> {
    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Client-ID"))?;

    let data_str = headers
        .get("X-Request")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "Missing X-Request"))?;

    let parts: Vec<&str> = data_str.split('|').collect();
    if parts.len() != 3 {
        return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
    }

    // potential enumeration issue, if thats something we want to cover.
    let shared_secret = {
        let config = state.config_rx.borrow();
        config
            .clients
            .get(client_id)
            .ok_or_else(|| {
                warn!("Unknown client '{}'", client_id);
                (StatusCode::FORBIDDEN, "Unknown client")
            })?
            .shared_secret
            .clone()
    };

    let command = match validate_hmac_message(data_str, shared_secret.as_ref()) {
        shuthost_common::HmacValidationResult::Valid(valid_message) => valid_message,
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

    let command_action: LeaseAction = serde_plain::from_str(&command)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid action in X-Request"))?;

    if command_action != expected_action {
        return Err((StatusCode::BAD_REQUEST, "Action mismatch"));
    }

    Ok(client_id.to_string())
}
