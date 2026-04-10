//! Web Push (VAPID) endpoints.
//!
//! Provides routes for exposing the VAPID public key, subscribing to
//! unscheduled-event push notifications, and sending a test notification.

use alloc::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::{TypedHeader, headers::ContentType};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, SubscriptionKeys, WebPushClient as _,
    WebPushMessageBuilder,
};

use crate::app::{AppState, db};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/vapid-public-key", get(get_vapid_public_key))
        .route(
            "/subscribe-host-unscheduled",
            get(check_host_unscheduled_subscription)
                .post(subscribe_host_unscheduled)
                .delete(unsubscribe_host_unscheduled),
        )
        .route("/test", post(send_test_push))
}

// ──────────────────────────────────────────────
// Request / response types
// ──────────────────────────────────────────────

#[derive(Deserialize)]
struct PushSubscriptionKeys {
    p256dh: String,
    auth: String,
}

#[derive(Deserialize)]
struct PushSubscriptionJson {
    endpoint: String,
    keys: PushSubscriptionKeys,
}

#[derive(Deserialize)]
struct SubscribeHostUnscheduledRequest {
    subscription: PushSubscriptionJson,
    hostname: String,
}

#[derive(Deserialize)]
struct CheckHostUnscheduledQuery {
    endpoint: String,
    hostname: String,
}

#[derive(Serialize)]
struct CheckHostUnscheduledResponse {
    subscribed: bool,
}

#[derive(Deserialize)]
struct UnsubscribeHostUnscheduledRequest {
    endpoint: String,
    hostname: String,
}

#[derive(Deserialize)]
struct TestPushRequest {
    hostname: String,
}

#[derive(Serialize)]
struct VapidPublicKeyResponse {
    #[serde(rename = "publicKey")]
    public_key: String,
}

// ──────────────────────────────────────────────
// Handlers
// ──────────────────────────────────────────────

/// Returns the VAPID public key as URL-safe base64 (no padding).
/// The frontend passes this to `PushManager.subscribe({ applicationServerKey })`.
#[axum::debug_handler]
async fn get_vapid_public_key(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref vapid_key) = state.vapid_key else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Push notifications require database persistence to be enabled",
        )
            .into_response();
    };

    let public_key_bytes = vapid_key.get_public_key();
    let public_key_b64 = URL_SAFE_NO_PAD.encode(&public_key_bytes);

    (
        TypedHeader(ContentType::json()),
        axum::Json(VapidPublicKeyResponse {
            public_key: public_key_b64,
        }),
    )
        .into_response()
}

/// Returns whether the given push endpoint is subscribed to unscheduled-event notifications
/// for the given host.
#[axum::debug_handler]
async fn check_host_unscheduled_subscription(
    State(state): State<AppState>,
    Query(params): Query<CheckHostUnscheduledQuery>,
) -> impl IntoResponse {
    let Some(ref pool) = state.db_pool else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match db::is_subscribed_to_host_unscheduled(pool, &params.endpoint, &params.hostname).await {
        Ok(subscribed) => axum::Json(CheckHostUnscheduledResponse { subscribed }).into_response(),
        Err(e) => {
            error!("Failed to check push subscription: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Removes the unscheduled-event subscription link for a specific endpoint + host pair.
#[axum::debug_handler]
async fn unsubscribe_host_unscheduled(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<UnsubscribeHostUnscheduledRequest>,
) -> impl IntoResponse {
    let Some(ref pool) = state.db_pool else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };

    if let Err(e) = db::unsubscribe_host_unscheduled(pool, &body.endpoint, &body.hostname).await {
        error!("Failed to unsubscribe from host unscheduled events: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::NO_CONTENT
}

/// Registers a browser push subscription for unscheduled-event notifications.
#[axum::debug_handler]
async fn subscribe_host_unscheduled(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<SubscribeHostUnscheduledRequest>,
) -> impl IntoResponse {
    let Some(ref pool) = state.db_pool else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };

    let sub_id = match db::upsert_push_subscription(
        pool,
        &body.subscription.endpoint,
        &body.subscription.keys.p256dh,
        &body.subscription.keys.auth,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to upsert push subscription: {e:#}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    if let Err(e) = db::subscribe_host_unscheduled(pool, sub_id, &body.hostname).await {
        error!("Failed to subscribe to host unscheduled events: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::NO_CONTENT
}

/// Sends a test push notification to all subscriptions watching the given host.
#[axum::debug_handler]
async fn send_test_push(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<TestPushRequest>,
) -> impl IntoResponse {
    let Some(ref pool) = state.db_pool else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    let Some(ref vapid_key) = state.vapid_key else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };

    let subscriptions = match db::get_subscriptions_for_host_unscheduled(pool, &body.hostname).await
    {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to fetch subscriptions: {e:#}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let payload = serde_json::json!({
        "title": "ShutHost",
        "body": format!("{} is online", body.hostname),
        "data": { "hostname": body.hostname },
    })
    .to_string();

    send_push_notifications(vapid_key, pool, &subscriptions, &payload).await;

    StatusCode::NO_CONTENT
}

// ──────────────────────────────────────────────
// Shared push-sending helper
// ──────────────────────────────────────────────

/// Sends `payload` as a push notification to each subscription in `subscriptions`.
/// Subscriptions that return 404/410 (expired) are removed from the database.
pub(crate) async fn send_push_notifications(
    vapid_key: &Arc<web_push::PartialVapidSignatureBuilder>,
    pool: &db::DbPool,
    subscriptions: &[db::PushSubscription],
    payload: &str,
) {
    let client = match IsahcWebPushClient::new() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create web push client: {e:?}");
            return;
        }
    };

    for sub in subscriptions {
        send_one_push_notification(&client, vapid_key, pool, sub, payload).await;
    }
}

async fn send_one_push_notification(
    client: &IsahcWebPushClient,
    vapid_key: &Arc<web_push::PartialVapidSignatureBuilder>,
    pool: &db::DbPool,
    sub: &db::PushSubscription,
    payload: &str,
) {
    let subscription_info = SubscriptionInfo {
        endpoint: sub.endpoint.clone(),
        keys: SubscriptionKeys {
            p256dh: sub.p256dh.clone(),
            auth: sub.auth.clone(),
        },
    };

    let sig = match vapid_key
        .as_ref()
        .clone()
        .add_sub_info(&subscription_info)
        .build()
    {
        Ok(s) => s,
        Err(e) => {
            error!(endpoint = %sub.endpoint, "Failed to build VAPID signature: {e:?}");
            return;
        }
    };

    let mut builder = WebPushMessageBuilder::new(&subscription_info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
    builder.set_vapid_signature(sig);

    let message = match builder.build() {
        Ok(m) => m,
        Err(e) => {
            error!(endpoint = %sub.endpoint, "Failed to build push message: {e:?}");
            return;
        }
    };

    match client.send(message).await {
        Ok(()) => {}
        Err(
            web_push::WebPushError::EndpointNotValid(_)
            | web_push::WebPushError::EndpointNotFound(_),
        ) => {
            warn!(endpoint = %sub.endpoint, "Push subscription expired, removing");
            if let Err(e) = db::delete_push_subscription(pool, &sub.endpoint).await {
                error!("Failed to delete expired subscription: {e:#}");
            }
        }
        Err(e) => {
            error!(endpoint = %sub.endpoint, "Failed to send push notification: {e:?}");
        }
    }
}
