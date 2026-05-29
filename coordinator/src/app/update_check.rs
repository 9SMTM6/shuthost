//! Periodic check for newer GitHub releases.

use core::time::Duration;

use semver::Version;
use tokio::time::{MissedTickBehavior, interval};
use tracing::debug;

use crate::{VERSION, app::AppState};

use super::state::LatestReleaseInfo;

pub(super) async fn check_for_updates_loop(state: AppState) {
    let mut ticker = interval(Duration::from_hours(24));
    // don't flood the API
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        do_check(&state).await;
        ticker.tick().await;
    }
}

async fn do_check(state: &AppState) {
    if !state.config_rx.borrow().server.check_for_updates {
        return;
    }

    match fetch_latest_release().await {
        Ok((tag_name, url)) => {
            let latest_str = tag_name.strip_prefix('v').unwrap_or(&tag_name);
            let current_base = VERSION.split('-').next().unwrap_or(VERSION);
            let current_str = current_base.strip_prefix('v').unwrap_or(current_base);

            match (Version::parse(latest_str), Version::parse(current_str)) {
                (Ok(latest), Ok(current)) if latest > current => {
                    tracing::warn!("Update available: {} \u{2192} {tag_name} ({url})", VERSION);
                    *state.latest_release.write().await = Some(LatestReleaseInfo { tag_name, url });
                }
                (Ok(_), Ok(_)) => {
                    *state.latest_release.write().await = None;
                }
                _ => {
                    debug!("Could not compare versions: current={VERSION}, latest={tag_name}");
                }
            }
        }
        Err(e) => {
            debug!("Update check failed: {e}");
        }
    }
}

async fn fetch_latest_release() -> eyre::Result<(String, String)> {
    #[derive(serde::Deserialize)]
    struct GithubRelease {
        tag_name: String,
        html_url: String,
    }

    let client = reqwest::Client::builder().user_agent("shuthost").build()?;

    let text = client
        .get("https://api.github.com/repos/9smtm6/shuthost/releases/latest")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let release = serde_json::from_str::<GithubRelease>(&text)?;
    Ok((release.tag_name, release.html_url))
}
