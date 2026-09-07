//! Opt-in package threat intelligence.
//!
//! OFF BY DEFAULT. When the user enables it, package names (and nothing else)
//! are sent to api.osv.dev for known-vulnerability lookups and to
//! registry.npmjs.org for publish-date checks. This is the ONLY code path in
//! Tracon that talks to the network, and it never transmits audit data.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracon_adapters::packages::{parse_specs, PackageSpec};
use tracon_core::store::Store;

use crate::intel_names::{encode_path_segment, is_valid_name};

pub const SETTING_KEY: &str = "threat_intel_enabled";
const CYCLE: Duration = Duration::from_secs(60);
const EVENTS_PER_CYCLE: i64 = 25;
const SPECS_PER_EVENT: usize = 5;
const FRESH_PACKAGE_WINDOW: time::Duration = time::Duration::hours(48);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_ATTEMPTS: u32 = 5;
const BACKOFF_BASE: Duration = Duration::from_secs(60);
const BACKOFF_CAP: Duration = Duration::from_secs(30 * 60);

pub fn is_enabled(store: &Store) -> bool {
    store
        .setting(SETTING_KEY)
        .ok()
        .flatten()
        .is_some_and(|v| v == "true")
}

/// Long-running worker: every cycle, if the user has opted in, check pending
/// package events. Transient failures back off per event and give up after
/// MAX_ATTEMPTS so one dead registry cannot pin the queue forever.
pub async fn run_worker(store: Arc<Store>) {
    let client = match reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent("tracon (https://github.com/tracon-dev/tracon)")
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut retries = RetryState::default();
    loop {
        if is_enabled(&store) {
            check_pending(&store, &client, &mut retries).await;
        }
        tokio::time::sleep(CYCLE).await;
    }
}

/// Per-event retry bookkeeping, in memory only: a restart simply retries.
#[derive(Default)]
struct RetryState {
    attempts: HashMap<i64, u32>,
    not_before: HashMap<i64, Instant>,
}

impl RetryState {
    fn is_waiting(&self, id: i64) -> bool {
        self.not_before
            .get(&id)
            .is_some_and(|until| Instant::now() < *until)
    }

    /// Returns true when the event has used up its attempts.
    fn record_failure(&mut self, id: i64) -> bool {
        let attempt = self.attempts.entry(id).or_insert(0);
        *attempt += 1;
        if *attempt >= MAX_ATTEMPTS {
            self.forget(id);
            return true;
        }
        let delay = backoff_delay(*attempt, random_unit());
        self.not_before.insert(id, Instant::now() + delay);
        false
    }

    fn forget(&mut self, id: i64) {
        self.attempts.remove(&id);
        self.not_before.remove(&id);
    }
}

/// Exponential backoff with up to +50% jitter so many stalled events do not
/// all retry in the same second. `jitter` is in [0, 1).
pub fn backoff_delay(attempt: u32, jitter: f64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16);
    let base = BACKOFF_BASE
        .saturating_mul(1u32 << exponent)
        .min(BACKOFF_CAP);
    base.mul_f64(1.0 + 0.5 * jitter.clamp(0.0, 0.999))
}

fn random_unit() -> f64 {
    let mut bytes = [0u8; 4];
    if getrandom::fill(&mut bytes).is_err() {
        return 0.0;
    }
    u32::from_le_bytes(bytes) as f64 / (u32::MAX as f64 + 1.0)
}

async fn check_pending(store: &Store, client: &reqwest::Client, retries: &mut RetryState) {
    let Ok(pending) = store.unchecked_package_events(EVENTS_PER_CYCLE) else {
        return;
    };
    for (id, ts, summary) in pending {
        if retries.is_waiting(id) {
            continue;
        }
        match assess_install(client, &ts, &summary).await {
            Ok(finding) => {
                if let Some(flag) = finding {
                    let _ = store.set_flag(id, &flag);
                }
                let _ = store.mark_intel_checked(id);
                retries.forget(id);
            }
            Err(_) => {
                let exhausted = retries.record_failure(id);
                if exhausted {
                    let _ = store.mark_intel_checked(id);
                }
            }
        }
    }
}

/// The worst finding across all packages in one install command, or None.
/// Names that do not fit their ecosystem's grammar are never looked up.
async fn assess_install(
    client: &reqwest::Client,
    event_ts: &str,
    summary: &str,
) -> anyhow::Result<Option<String>> {
    let specs = parse_specs(summary)
        .into_iter()
        .filter(|spec| is_valid_name(&spec.ecosystem, &spec.name))
        .take(SPECS_PER_EVENT);
    for spec in specs {
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

/// Send a request and parse the JSON body. A 4xx means the registry has
/// nothing for this name (unknown package, bad request): that is a final
/// answer, so it yields Ok(None) rather than an error that would retry.
/// 5xx and transport failures are errors and go through backoff.
async fn fetch_json(request: reqwest::RequestBuilder) -> anyhow::Result<Option<Value>> {
    let response = request.send().await?;
    let status = response.status();
    if status.is_client_error() {
        return Ok(None);
    }
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        anyhow::bail!("registry answered {status}");
    }
    Ok(Some(response.json().await?))
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
    let request = client.post("https://api.osv.dev/v1/query").json(&body);
    let Some(response) = fetch_json(request).await? else {
        return Ok(None);
    };
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
    let url = format!(
        "https://registry.npmjs.org/{}",
        encode_path_segment(&spec.name)
    );
    let Some(response) = fetch_json(client.get(&url)).await? else {
        return Ok(false);
    };
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

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("tracon-test")
            .build()
            .unwrap()
    }

    /// Hits the real OSV API with a synthetic known-bad package; excluded
    /// from normal runs. Manual check: cargo test -p tracon-ingest -- --ignored
    #[tokio::test]
    #[ignore]
    async fn flags_the_event_stream_backdoor_via_osv() {
        let flag = assess_install(
            &test_client(),
            "2026-08-29T00:00:00Z",
            "npm install event-stream@3.3.6",
        )
        .await
        .unwrap();
        assert!(flag.unwrap().contains("known vulnerabilities"));
    }

    /// A client that can only reach a closed port: any network use would fail
    /// loudly, so Ok(None) proves the invalid name never left the process.
    #[tokio::test]
    async fn invalid_names_are_skipped_without_network() {
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all("http://127.0.0.1:9").unwrap())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let result = assess_install(
            &client,
            "2026-08-29T00:00:00Z",
            "npm install ../../evil@1.0.0 Not-Lower@2.0.0",
        )
        .await
        .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        assert_eq!(backoff_delay(1, 0.0), Duration::from_secs(60));
        assert_eq!(backoff_delay(2, 0.0), Duration::from_secs(120));
        assert_eq!(backoff_delay(3, 0.0), Duration::from_secs(240));
        assert_eq!(backoff_delay(40, 0.0), BACKOFF_CAP);
        // Jitter only ever lengthens the wait, by at most half.
        let jittered = backoff_delay(1, 0.999);
        assert!(jittered > Duration::from_secs(60));
        assert!(jittered <= Duration::from_secs(90));
    }

    #[test]
    fn retry_state_gives_up_after_max_attempts() {
        let mut retries = RetryState::default();
        for _ in 0..MAX_ATTEMPTS - 1 {
            assert!(!retries.record_failure(7));
            assert!(retries.is_waiting(7));
        }
        assert!(retries.record_failure(7));
        assert!(!retries.is_waiting(7));
    }
}
