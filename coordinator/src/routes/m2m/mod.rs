use std::{collections::{HashMap, HashSet}, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response}, routing::post, Json,
};
use rand::seq::IndexedRandom;
use serde::Deserialize;
use serde_json::json;
use shuthost_common::{is_timestamp_in_valid_range, verify_hmac};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::http::AppState;

const CLIENT_SCRIPT_TEMPLATE: &str = include_str!("shuthost_client.sh");

// Word lists for generating readable client IDs
const ADJECTIVES: &[&str] = &[
    "red", "blue", "swift", "calm", "bold", "wise", "kind", "brave",
];
const NOUNS: &[&str] = &[
    "fox", "bird", "wolf", "bear", "lion", "deer", "hawk", "eagle",
];

#[derive(Deserialize)]
pub struct ClientScriptQuery {
    remote_url: String,
    #[serde(default)]
    client_id: Option<String>,
}

pub async fn download_client_script(Query(params): Query<ClientScriptQuery>) -> impl IntoResponse {
    // TODO: switch to download per script like with the agent.
    // Like with the agent, infer the client name from hostname, and print out the config to hadd to the config file.
    // Both of these have to happen in the download script.
    let client_id = params.client_id.unwrap_or_else(generate_client_id);
    let shared_secret = shuthost_common::generate_secret();

    let script = CLIENT_SCRIPT_TEMPLATE
        .replace("{embedded_remote_url}", &params.remote_url)
        .replace("{client_id}", &client_id)
        .replace("{shared_secret}", &shared_secret);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .header(
            "Content-Disposition",
            "attachment; filename=\"shuthost-client.sh\"",
        )
        .body(script)
        .unwrap()
}

pub fn m2m_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/lease/{hostname}/{action}", post(handle_lease))
        .route("/test_wol", post(test_wol))
}

fn generate_client_id() -> String {
    let mut rng = rand::rng();
    let adjective = ADJECTIVES.choose(&mut rng).unwrap();
    let noun = NOUNS.choose(&mut rng).unwrap();
    format!("{}-{}", adjective, noun)
}

async fn test_wol(
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let remote_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
        })
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "No client IP found").into_response())?;

    match crate::wol::test_wol_reachability(remote_ip) {
        Ok((direct, broadcast)) => {
            Ok(Json(json!({
                "direct": direct,
                "broadcast": broadcast
            })).into_response())
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e).into_response()),
    }
}



/// node_name => set of client_ids holding lease
pub type LeaseMap = Arc<Mutex<HashMap<String, HashSet<String>>>>;

#[axum::debug_handler]
async fn handle_lease(
    Path((node, action)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_id = headers
        .get("X-Client-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing X-Client-ID"))?;

    let data_str = headers
        .get("X-Request")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing X-Request"))?;

    let parts: Vec<&str> = data_str.split('|').collect();
    if parts.len() != 3 {
        return Err((StatusCode::BAD_REQUEST, "Invalid request format"));
    }

    let (timestamp_str, command, signature) = (parts[0], parts[1], parts[2]);
    if command != action {
        return Err((StatusCode::BAD_REQUEST, "Action mismatch"));
    }

    let timestamp: u64 = timestamp_str
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid timestamp"))?;

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

    if !is_timestamp_in_valid_range(timestamp) {
        info!("Timestamp out of range for client '{}'", client_id);
        return Err((StatusCode::UNAUTHORIZED, "Timestamp out of range"));
    }

    let message = format!("{}|{}", timestamp_str, command);
    if !verify_hmac(&message, signature, &shared_secret) {
        info!("Invalid HMAC signature for client '{}'", client_id);
        return Err((StatusCode::UNAUTHORIZED, "Invalid HMAC signature"));
    }

    let mut leases = state.leases.lock().await;
    let lease_set = leases.entry(node.clone()).or_default();

    // TODO: Implement taking actual action based on leases (shutdown etc)

    match action.as_str() {
        "take" => {
            lease_set.insert(client_id.to_string());
            info!("Client '{}' took lease on '{}'", client_id, node);
            Ok("Lease taken".into_response())
        }
        "release" => {
            lease_set.remove(client_id);
            info!("Client '{}' released lease on '{}'", client_id, node);
            Ok("Lease released".into_response())
        }
        _ => Err((StatusCode::BAD_REQUEST, "Invalid action")),
    }
}
