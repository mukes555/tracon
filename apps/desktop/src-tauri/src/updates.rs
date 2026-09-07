//! Opt-in version check against the public GitHub release feed. Nothing
//! about the user or their data is sent; the reply is cached in settings so
//! the UI never waits on the network, and the install hint depends on how
//! the app got here (Homebrew or a direct download).
use std::sync::Arc;

use serde::Serialize;
use tracon_core::store::Store;

const LATEST_URL: &str = "https://api.github.com/repos/mukes555/tracon/releases/latest";
const RELEASES_URL: &str = "https://github.com/mukes555/tracon/releases/latest";
const SETTING_ENABLED: &str = "update_check";
const SETTING_LATEST: &str = "update_latest";
const SETTING_URL: &str = "update_url";
const SETTING_CHECKED_AT: &str = "update_checked_at";

#[derive(Serialize, Clone)]
pub struct UpdateStatus {
    pub current: String,
    pub latest: Option<String>,
    pub url: Option<String>,
    pub checked_at: Option<String>,
    pub enabled: bool,
    pub via_brew: bool,
    /// True when `latest` is newer than `current`.
    pub available: bool,
}

pub fn status(store: &Store, current: &str) -> UpdateStatus {
    let latest = store.setting(SETTING_LATEST).ok().flatten();
    let available = latest.as_deref().is_some_and(|l| is_newer(l, current));
    UpdateStatus {
        current: current.to_string(),
        url: store
            .setting(SETTING_URL)
            .ok()
            .flatten()
            .or_else(|| Some(RELEASES_URL.to_string())),
        checked_at: store.setting(SETTING_CHECKED_AT).ok().flatten(),
        enabled: enabled(store),
        via_brew: installed_via_brew(),
        latest,
        available,
    }
}

pub fn enabled(store: &Store) -> bool {
    store
        .setting(SETTING_ENABLED)
        .ok()
        .flatten()
        .is_some_and(|v| v == "true")
}

/// One check: fetch, compare, cache. Errors leave the cached values alone.
pub async fn check_now(store: &Store, current: &str) -> anyhow::Result<UpdateStatus> {
    let (version, url) = fetch_latest(current).await?;
    store.set_setting(SETTING_LATEST, &version)?;
    store.set_setting(SETTING_URL, &url)?;
    store.set_setting(SETTING_CHECKED_AT, &tracon_core::clock::now_ts())?;
    Ok(status(store, current))
}

async fn fetch_latest(current: &str) -> anyhow::Result<(String, String)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("tracon/{current}"))
        .build()?;
    let body: serde_json::Value = client
        .get(LATEST_URL)
        .header("accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let tag = body["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("release feed had no tag_name"))?;
    let url = body["html_url"]
        .as_str()
        .unwrap_or(RELEASES_URL)
        .to_string();
    Ok((tag.trim_start_matches('v').to_string(), url))
}

/// Five minutes after launch, then daily, while the setting is on.
pub fn spawn_update_worker(store: Arc<Store>, current: String) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;
        loop {
            if enabled(&store) {
                if let Err(err) = check_now(&store, &current).await {
                    log::warn!("update check failed: {err}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
        }
    });
}

/// Homebrew keeps a Caskroom entry for every cask it installed.
pub fn installed_via_brew() -> bool {
    [
        "/opt/homebrew/Caskroom/tracon",
        "/usr/local/Caskroom/tracon",
    ]
    .iter()
    .any(|p| std::path::Path::new(p).is_dir())
}

/// Numeric dotted compare; a pre-release suffix on either side is ignored.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(latest), parse(current));
    let len = a.len().max(b.len());
    for i in 0..len {
        let (x, y) = (*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0));
        if x != y {
            return x > y;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_dotted_versions() {
        assert!(is_newer("0.4.0", "0.3.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("v0.3.1", "0.3.0"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("0.2.9", "0.3.0"));
        assert!(!is_newer("0.3.0-rc1", "0.3.0"));
    }
}
