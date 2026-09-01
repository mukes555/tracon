//! Opt-in package threat intelligence.
//!
//! OFF BY DEFAULT. When the user enables it, package names (and nothing else)
//! are sent to api.osv.dev for known-vulnerability lookups and to
//! registry.npmjs.org for publish-date checks. This is the ONLY code path in
//! Tracon that talks to the network, and it never transmits audit data.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracon_adapters::packages::{parse_specs, PackageSpec};
use tracon_core::store::Store;

pub const SETTING_KEY: &str = "threat_intel_enabled";
const CYCLE: Duration = Duration::from_secs(60);
const EVENTS_PER_CYCLE: i64 = 25;
const SPECS_PER_EVENT: usize = 5;
const FRESH_PACKAGE_WINDOW: time::Duration = time::Duration::hours(48);

pub fn is_enabled(store: &Store) -> bool {
    store
        .setting(SETTING_KEY)
        .ok()
        .flatten()
        .is_some_and(|v| v == "true")
}

/// Long-running worker: every cycle, if the user has opted in, check pending
/// package events. Network errors leave events unchecked so they retry.
pub async fn run_worker(store: Arc<Store>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("tracon (https://github.com/tracon-dev/tracon)")
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    loop {
        if is_enabled(&store) {
            check_pending(&store, &client).await;
        }
        tokio::time::sleep(CYCLE).await;
    }
}

async fn check_pending(store: &Store, client: &reqwest::Client) {
    let Ok(pending) = store.unchecked_package_events(EVENTS_PER_CYCLE) else {
        return;
    };
    for (id, ts, summary) in pending {
        match assess_install(client, &ts, &summary).await {
            Ok(Some(flag)) => {
                let _ = store.set_flag(id, &flag);
                let _ = store.mark_intel_checked(id);
            }
            Ok(None) => {
                let _ = store.mark_intel_checked(id);
            }
            // Network trouble: leave unchecked, a later cycle retries.
            Err(_) => {}
        }
    }
}

/// The worst finding across all packages in one install command, or None.
async fn assess_install(
    client: &reqwest::Client,
    event_ts: &str,
    summary: &str,
) -> anyhow::Result<Option<String>> {
    let specs = parse_specs(summary);
    for spec in specs.into_iter().take(SPECS_PER_EVENT) {
        if let Some(flag) = assess_spec(client, event_ts, &spec).await? {
            return Ok(Some(flag));
        }
    }
    Ok(None)
}

async fn assess_spec(
    client: &reqwest::Client,
    event_ts: &str,
    spec: &PackageSpec,
) -> anyhow::Result<Option<String>> {
    if let Some(vuln_count) = osv_vuln_count(client, spec).await? {
        if vuln_count > 0 {
            return Ok(Some(format!(
                "{}: {vuln_count} known vulnerabilities (OSV)",
                spec.name
            )));
        }
    }
    if spec.ecosystem == "npm" && npm_version_is_fresh(client, event_ts, spec).await? {
        return Ok(Some(format!(
            "{}: version published under 48h before install",
            spec.name
        )));
    }
    Ok(None)
}

/// OSV only gives precise answers for exact versions, so unpinned installs
/// are skipped rather than reported noisily across all historical versions.
async fn osv_vuln_count(
    client: &reqwest::Client,
    spec: &PackageSpec,
) -> anyhow::Result<Option<u64>> {
    let Some(version) = &spec.version else {
        return Ok(None);
    };
    let body = json!({
        "version": version,
        "package": { "name": spec.name, "ecosystem": spec.ecosystem }
    });
    let response: Value = client
        .post("https://api.osv.dev/v1/query")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let count = response
        .get("vulns")
        .and_then(Value::as_array)
        .map(|v| v.len() as u64)
        .unwrap_or(0);
    Ok(Some(count))
}

/// The Aikido-style freshness gate: a version published less than 48 hours
/// before the agent installed it is a classic compromise window.
async fn npm_version_is_fresh(
    client: &reqwest::Client,
    event_ts: &str,
    spec: &PackageSpec,
) -> anyhow::Result<bool> {
    let Some(version) = &spec.version else {
        return Ok(false);
    };
    let url = format!("https://registry.npmjs.org/{}", spec.name);
    let response: Value = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(published) = response
        .get("time")
        .and_then(|t| t.get(version))
        .and_then(Value::as_str)
    else {
        return Ok(false);
    };

    let (Ok(installed_at), Ok(published_at)) = (
        OffsetDateTime::parse(event_ts, &Rfc3339),
        OffsetDateTime::parse(published, &Rfc3339),
    ) else {
        return Ok(false);
    };
    Ok(installed_at - published_at < FRESH_PACKAGE_WINDOW)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits the real OSV API with a synthetic known-bad package; excluded
    /// from normal runs. Manual check: cargo test -p tracon-ingest -- --ignored
    #[tokio::test]
    #[ignore]
    async fn flags_the_event_stream_backdoor_via_osv() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("tracon-test")
            .build()
            .unwrap();
        let flag = assess_install(
            &client,
            "2026-08-29T00:00:00Z",
            "npm install event-stream@3.3.6",
        )
        .await
        .unwrap();
        assert!(flag.unwrap().contains("known vulnerabilities"));
    }
}
