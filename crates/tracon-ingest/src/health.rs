//! Process-wide visibility for store write failures.
//!
//! The ingest paths deliberately never surface errors to agents (hooks always
//! get 200, tailing and spooling are best-effort), so a failing database would
//! otherwise be invisible. Every discarded insert error lands here instead.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub struct Health {
    pub insert_failures: AtomicU64,
    pub last_error: Mutex<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthSnapshot {
    pub insert_failures: u64,
    pub last_error: Option<String>,
}

static HEALTH: Health = Health {
    insert_failures: AtomicU64::new(0),
    last_error: Mutex::new(String::new()),
};

pub fn record_insert_failure(err: &anyhow::Error) {
    HEALTH.insert_failures.fetch_add(1, Ordering::Relaxed);
    let mut last = HEALTH.last_error.lock().unwrap_or_else(|e| e.into_inner());
    *last = err.to_string();
}

pub fn health() -> HealthSnapshot {
    let last = HEALTH.last_error.lock().unwrap_or_else(|e| e.into_inner());
    let last_error = if last.is_empty() {
        None
    } else {
        Some(last.clone())
    };
    HealthSnapshot {
        insert_failures: HEALTH.insert_failures.load(Ordering::Relaxed),
        last_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_are_counted_and_last_message_kept() {
        let before = health().insert_failures;
        record_insert_failure(&anyhow::anyhow!("disk full"));
        let after = health();
        assert_eq!(after.insert_failures, before + 1);
        assert_eq!(after.last_error.as_deref(), Some("disk full"));
    }
}
