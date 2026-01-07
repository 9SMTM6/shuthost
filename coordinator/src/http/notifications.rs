//! Push notification functionality.
//!
//! This module handles sending push notifications to subscribed clients.

use crate::db;
use serde::Serialize;

/// Different types of push notifications that can be sent
///
/// To add a new notification type:
/// 1. Add a new variant to this enum with its data struct
/// 2. Create a corresponding data struct (e.g., `NewTypeData`)
/// 3. Update the TypeScript types in the service worker
/// 4. Add handling logic in the service worker if needed
#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
enum NotificationType {
    /// Notification about host status changes (online/offline)
    HostStatus(HostStatusData),
}

/// Data for host status notifications
#[derive(Serialize)]
struct HostStatusData {
    host: String,
    action: String,
}

/// Helper function to create a host status notification
fn create_host_status_notification(host_name: &str, action: &str) -> NotificationType {
    NotificationType::HostStatus(HostStatusData {
        host: host_name.to_string(),
        action: action.to_string(),
    })
}

/// Sends push notifications to all subscribers about a host coming online.
///
/// # Arguments
///
/// * `pool` - Database connection pool.
/// * `host_name` - The name of the host that came online.
///
/// # Errors
///
/// Returns an error if VAPID keys cannot be retrieved or push sending fails.
pub(crate) async fn send_host_online(pool: &db::DbPool, host_name: &str) -> eyre::Result<()> {
    let vapid_keys = db::get_or_generate_vapid_keys(pool).await?;
    let subscriptions = db::get_push_subscriptions(pool).await?;

    if subscriptions.is_empty() {
        return Ok(());
    }

    let message = create_host_status_notification(host_name, "online");

    for subscription in subscriptions {
        let subscription_info = web_push::SubscriptionInfo::new(
            subscription.endpoint.clone(),
            subscription.p256dh,
            subscription.auth,
        );

        let vapid_key = web_push::VapidSignatureBuilder::from_pem(
            vapid_keys.private_key.as_bytes(),
            &subscription_info,
        )?;
        let vapid_signature = vapid_key.build()?;

        let message_content = serde_json::to_vec(&message)?;
        let mut builder = web_push::WebPushMessageBuilder::new(&subscription_info);
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, &message_content);
        builder.set_vapid_signature(vapid_signature);

        #[expect(clippy::shadow_unrelated, reason = "false positive")]
        let message = builder.build()?;
        // this is a false negative because of pub(crate)
        // #[expect(clippy::missing_panics_doc, reason = "the payload was set above")]
        let message_bytes = message
            .payload
            .as_ref()
            .expect("The payload was set above")
            .content
            .clone();

        // Send the HTTP request
        let client = reqwest::Client::new();
        let response = client
            .post(&subscription.endpoint)
            .header("TTL", "86400")
            .header("Content-Type", "application/octet-stream")
            .header("Content-Encoding", "aes128gcm")
            .body(message_bytes)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    "Push notification sent successfully to {} for host {}",
                    subscription.endpoint,
                    host_name
                );
            }
            Ok(resp) => {
                tracing::warn!(
                    "Push notification failed for {}: status {}",
                    subscription.endpoint,
                    resp.status()
                );
            }
            Err(e) => {
                tracing::error!(
                    "Failed to send push notification to {}: {}",
                    subscription.endpoint,
                    e
                );
            }
        }
    }

    Ok(())
}
