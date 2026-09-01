use std::sync::Arc;
use std::time::Duration;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracon_core::store::Store;

pub const SETTING_KEY: &str = "retention_days";
const DEFAULT_RETENTION_DAYS: i64 = 90;
const CYCLE: Duration = Duration::from_secs(6 * 60 * 60);

pub fn retention_days(store: &Store) -> i64 {
    store
        .setting(SETTING_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .filter(|d| *d > 0)
        .unwrap_or(DEFAULT_RETENTION_DAYS)
}

/// Purge events past the retention window, once at startup and then every
/// few hours. Keeps the local database bounded; retention only ever deletes
/// Tracon's own data, never agent files.
pub async fn run_worker(store: Arc<Store>) {
    loop {
        purge_once(&store);
        tokio::time::sleep(CYCLE).await;
    }
}

pub fn purge_once(store: &Store) -> usize {
    let days = retention_days(store);
    let cutoff = OffsetDateTime::now_utc() - time::Duration::days(days);
    let Ok(cutoff) = cutoff.format(&Rfc3339) else {
        return 0;
    };
    store.purge_events_before(&cutoff).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracon_core::event::{AgentEvent, EventKind, EventSource};

    fn event_at(ts: &str) -> AgentEvent {
        AgentEvent {
            id: None,
            agent: "claude-code".into(),
            session_id: "s".into(),
            ts: ts.into(),
            kind: EventKind::ToolCall,
            source: EventSource::Hook,
            cwd: None,
            tool_name: Some("Bash".into()),
            summary: Some("ls".into()),
            flag: None,
            payload: serde_json::Value::Null,
            dedupe_key: None,
        }
    }

    #[test]
    fn purges_only_events_past_the_window() {
        let store = Store::open_in_memory().unwrap();
        store.insert(&event_at("2020-01-01T00:00:00Z")).unwrap();
        store.insert(&event_at(&tracon_core::now_iso())).unwrap();

        let removed = purge_once(&store);
        assert_eq!(removed, 1);
        assert_eq!(store.stats().unwrap().event_count, 1);
    }

    #[test]
    fn retention_setting_overrides_default() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(retention_days(&store), 90);
        store.set_setting(SETTING_KEY, "30").unwrap();
        assert_eq!(retention_days(&store), 30);
        store.set_setting(SETTING_KEY, "junk").unwrap();
        assert_eq!(retention_days(&store), 90);
    }
}
