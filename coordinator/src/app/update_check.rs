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
        ticker.tick().await;
        do_check(&state).await;
    }
}

async fn do_check(state: &AppState) {
    if !state.config_rx.borrow().server.check_for_updates {
        return;
    }

    match fetch_latest_release().await {
        Ok((tag_name, url)) => match needs_update(VERSION, &tag_name) {
            Some(true) => {
                tracing::warn!("Update available: {} \u{2192} {tag_name} ({url})", VERSION);
                *state.latest_release.write().await = Some(LatestReleaseInfo { tag_name, url });
            }
            Some(false) => {
                *state.latest_release.write().await = None;
            }
            None => {
                *state.latest_release.write().await = None;
                debug!("Could not compare versions: current={VERSION}, latest={tag_name}");
            }
        },
        Err(e) => {
            debug!("Update check failed: {e}");
        }
    }
}

/// Returns `Some(true)` if `latest_tag` is a higher semver than `current`,
/// `Some(false)` if `current` is already at or ahead of it,
/// or `None` if either string cannot be parsed as semver (e.g. bare commit hashes).
///
/// Strips a leading `v` and any git-describe suffix (e.g. `-3-gabcdef`) before parsing.
fn needs_update(current: &str, latest_tag: &str) -> Option<bool> {
    let latest_str = latest_tag.strip_prefix('v').unwrap_or(latest_tag);
    let current_base = current.split('-').next().unwrap_or(current);
    let current_str = current_base.strip_prefix('v').unwrap_or(current_base);

    let latest = Version::parse(latest_str).ok()?;
    let current = Version::parse(current_str).ok()?;
    Some(latest > current)
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
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let release = serde_json::from_str::<GithubRelease>(&text)?;
    Ok((release.tag_name, release.html_url))
}

#[cfg(test)]
mod tests {
    use super::needs_update;

    #[test]
    fn update_available() {
        assert_eq!(needs_update("v1.8.1", "v1.9.0"), Some(true));
    }

    #[test]
    fn already_up_to_date() {
        assert_eq!(needs_update("v1.8.1", "v1.8.1"), Some(false));
    }

    #[test]
    fn running_newer_than_latest_release() {
        assert_eq!(needs_update("v1.9.0", "v1.8.1"), Some(false));
    }

    #[test]
    fn git_describe_suffix_stripped() {
        assert_eq!(needs_update("v1.8.1-3-gabcdef", "v1.9.0"), Some(true));
        assert_eq!(needs_update("v1.8.1-3-gabcdef", "v1.8.1"), Some(false));
    }

    #[test]
    fn unparsable_current_returns_none() {
        assert_eq!(needs_update("gabcdef", "v1.9.0"), None);
        assert_eq!(needs_update("unknown", "v1.9.0"), None);
    }

    #[test]
    fn unparsable_latest_returns_none() {
        assert_eq!(needs_update("v1.8.1", "not-a-version"), None);
    }

    #[test]
    fn patch_update_detected() {
        assert_eq!(needs_update("v1.8.1", "v1.8.2"), Some(true));
    }
}
