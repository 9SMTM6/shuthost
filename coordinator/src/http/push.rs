//! Web Push (VAPID) endpoints.
//!
//! Provides routes for exposing the VAPID public key and subscribing to
//! unscheduled-event push notifications.

use alloc::sync::Arc;
use core::time::Duration;
use tokio::time::sleep;

use axum::{
    Router,
    extract::{Query, State},
    response::{IntoResponse, Response},
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

macro_rules! require_db_pool {
    ($state:expr) => {{
        let Some(ref pool) = $state.db_pool else {
            return StatusCode::SERVICE_UNAVAILABLE;
        };
        pool
    }};
    (response; $state:expr) => {{
        let Some(ref pool) = $state.db_pool else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        pool
    }};
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/vapid-public-key", get(get_vapid_public_key))
        .route(
            "/subscribe-host-unscheduled",
            get(check_host_unscheduled_subscription)
                .post(subscribe_host_unscheduled)
                .delete(unsubscribe_host_unscheduled),
        )
        .route(
            "/subscribe-host-operation-failed",
            get(check_host_operation_failed_subscription)
                .post(subscribe_host_operation_failed)
                .delete(unsubscribe_host_operation_failed),
        )
        .route(
            "/subscribe-host-online-for",
            get(check_host_online_for_subscription)
                .post(subscribe_host_online_for)
                .delete(unsubscribe_host_online_for),
        )
        .route(
            "/subscribe-host-online-for-oneshot",
            post(subscribe_host_online_for_oneshot),
        )
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
struct HostSubscriptionRequest {
    subscription: PushSubscriptionJson,
    hostname: String,
}

#[derive(Deserialize)]
struct HostSubscriptionData {
    endpoint: String,
    hostname: String,
}

#[derive(Serialize)]
struct CheckHostSubscriptionResponse {
    subscribed: bool,
}

const MAX_HOST_ONLINE_FOR_DURATION_SECS: i64 = 86_400 * 30; // 30 days

fn validate_host_online_for_duration_secs(duration_secs: i64) -> Result<u64, Box<Response>> {
    if duration_secs <= 0 {
        return Err(Box::new(
            (
                StatusCode::BAD_REQUEST,
                "duration_secs must be greater than 0",
            )
                .into_response(),
        ));
    }
    if duration_secs > MAX_HOST_ONLINE_FOR_DURATION_SECS {
        return Err(Box::new(
            (
                StatusCode::BAD_REQUEST,
                "duration_secs must be 30 days or less",
            )
                .into_response(),
        ));
    }

    Ok(duration_secs.cast_unsigned())
}

#[derive(Deserialize)]
struct HostOnlineForRequest {
    subscription: PushSubscriptionJson,
    hostname: String,
    duration_secs: i64,
}

#[derive(Serialize)]
struct CheckHostOnlineForResponse {
    subscribed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_secs: Option<i64>,
}

#[derive(Serialize)]
pub(crate) struct HostSpecificNotificationData {
    pub(crate) hostname: String,
}

#[derive(Serialize)]
pub(crate) struct NotificationPayload<D = ()>
where
    D: Serialize,
{
    title: &'static str,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<D>,
}

impl<D> NotificationPayload<D>
where
    D: Serialize,
{
    pub(crate) const fn with_data(body: String, data: D) -> Self {
        Self {
            title: "ShutHost",
            body,
            data: Some(data),
        }
    }

    pub(crate) fn into_json(self) -> String {
        serde_json::to_string(&self).expect("push notification payload serialization must not fail")
    }
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
    Query(params): Query<HostSubscriptionData>,
) -> impl IntoResponse {
    let pool = require_db_pool!(response; state);

    match db::is_subscribed_to_host_unscheduled(pool, &params.endpoint, &params.hostname).await {
        Ok(subscribed) => axum::Json(CheckHostSubscriptionResponse { subscribed }).into_response(),
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
    axum::Json(body): axum::Json<HostSubscriptionData>,
) -> impl IntoResponse {
    let pool = require_db_pool!(state);

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
    axum::Json(body): axum::Json<HostSubscriptionRequest>,
) -> impl IntoResponse {
    let pool = require_db_pool!(state);

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

// ──────────────────────────────────────────────
// Handlers: host operation-failed subscriptions
// ──────────────────────────────────────────────

/// Returns whether the given push endpoint is subscribed to operation-failed notifications
/// for the given host.
#[axum::debug_handler]
async fn check_host_operation_failed_subscription(
    State(state): State<AppState>,
    Query(params): Query<HostSubscriptionData>,
) -> impl IntoResponse {
    let pool = require_db_pool!(response; state);

    match db::is_subscribed_to_host_operation_failed(pool, &params.endpoint, &params.hostname).await
    {
        Ok(subscribed) => axum::Json(CheckHostSubscriptionResponse { subscribed }).into_response(),
        Err(e) => {
            error!("Failed to check operation-failed push subscription: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Registers a browser push subscription for operation-failed notifications.
#[axum::debug_handler]
async fn subscribe_host_operation_failed(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<HostSubscriptionRequest>,
) -> impl IntoResponse {
    let pool = require_db_pool!(state);

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

    if let Err(e) =
        db::add_push_subscription_host_operation_failed(pool, sub_id, &body.hostname).await
    {
        error!("Failed to subscribe to host operation-failed events: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::NO_CONTENT
}

/// Removes the operation-failed subscription link for a specific endpoint + host pair.
#[axum::debug_handler]
async fn unsubscribe_host_operation_failed(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<HostSubscriptionData>,
) -> impl IntoResponse {
    let pool = require_db_pool!(state);

    if let Err(e) =
        db::unsubscribe_host_operation_failed(pool, &body.endpoint, &body.hostname).await
    {
        error!("Failed to unsubscribe from host operation-failed events: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::NO_CONTENT
}

// ──────────────────────────────────────────────
// Handlers: host online-for subscriptions
// ──────────────────────────────────────────────

/// Returns whether the given push endpoint is subscribed to online-for notifications
/// for the given host, and if so, the configured duration in seconds.
#[axum::debug_handler]
async fn check_host_online_for_subscription(
    State(state): State<AppState>,
    Query(params): Query<HostSubscriptionData>,
) -> impl IntoResponse {
    let pool = require_db_pool!(response; state);

    match db::is_subscribed_to_host_online_for(pool, &params.endpoint, &params.hostname).await {
        Ok(duration_secs) => axum::Json(CheckHostOnlineForResponse {
            subscribed: duration_secs.is_some(),
            duration_secs,
        })
        .into_response(),
        Err(e) => {
            error!("Failed to check online-for push subscription: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Registers a browser push subscription for recurring online-for notifications.
#[axum::debug_handler]
async fn subscribe_host_online_for(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<HostOnlineForRequest>,
) -> Response {
    let pool = require_db_pool!(response; state);

    if let Err(error_response) =
        validate_host_online_for_duration_secs(body.duration_secs).map(|_| ())
    {
        return *error_response;
    }

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
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Err(e) =
        db::subscribe_host_online_for(pool, sub_id, &body.hostname, body.duration_secs).await
    {
        error!("Failed to subscribe to host online-for events: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

/// Removes the online-for subscription link for a specific endpoint + host pair.
#[axum::debug_handler]
async fn unsubscribe_host_online_for(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<HostSubscriptionData>,
) -> impl IntoResponse {
    let pool = require_db_pool!(state);

    if let Err(e) = db::unsubscribe_host_online_for(pool, &body.endpoint, &body.hostname).await {
        error!("Failed to unsubscribe from host online-for events: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::NO_CONTENT
}

/// Registers a one-shot push notification that fires once the given host has been
/// online for `duration_secs` seconds. The host must currently be online; if not,
/// returns 409 Conflict.
///
/// Because one-shot subscriptions are not persisted, they are lost if the coordinator
/// restarts between subscription time and notification time.
#[axum::debug_handler]
async fn subscribe_host_online_for_oneshot(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<HostOnlineForRequest>,
) -> Response {
    let pool = require_db_pool!(response; state);
    let pool = pool.clone();

    let duration_secs = body.duration_secs;
    let duration = match validate_host_online_for_duration_secs(duration_secs) {
        Ok(duration) => duration,
        Err(error_response) => return *error_response,
    };

    let Some(vapid_key) = state.vapid_key.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let session_start = {
        let guard = state.online_since.read().await;
        let Some(&instant) = guard.get(&body.hostname) else {
            return StatusCode::CONFLICT.into_response();
        };
        instant
    };

    let sub = db::PushSubscription {
        endpoint: body.subscription.endpoint.clone(),
        p256dh: body.subscription.keys.p256dh.clone(),
        auth: body.subscription.keys.auth.clone(),
    };
    let duration = Duration::from_secs(duration);
    let online_since = state.online_since.clone();
    let hostname = body.hostname.clone();

    // Upsert the push subscription so future sends work correctly.
    if let Err(e) = db::upsert_push_subscription(
        &pool,
        &body.subscription.endpoint,
        &body.subscription.keys.p256dh,
        &body.subscription.keys.auth,
    )
    .await
    {
        error!("Failed to upsert push subscription for oneshot: {e:#}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    tokio::spawn(async move {
        sleep(duration).await;
        if online_since.read().await.get(&hostname) != Some(&session_start) {
            return;
        }
        let payload = NotificationPayload::with_data(
            format!("{hostname} has been online for {duration_secs} seconds"),
            HostSpecificNotificationData { hostname },
        )
        .into_json();
        send_push_notifications(&vapid_key, &pool, &[sub], &payload).await;
    });

    StatusCode::NO_CONTENT.into_response()
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
