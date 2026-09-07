//! The single write path for events entering the store from this crate.
//!
//! Adapters are trusted to normalize, not to bound: a hook payload can carry
//! a multi-megabyte tool result and a summary can be a whole script. Bounding
//! here, once, keeps the database and the timeline UI sane whichever capture
//! path an event took.

use tracon_core::event::AgentEvent;
use tracon_core::store::Store;

pub const MAX_SUMMARY_CHARS: usize = 300;
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Bound and insert one event. Ok(true) means newly stored, Ok(false) means
/// the dedupe key already existed. Errors are recorded in `health` and
/// returned so callers can decide whether to advance offsets.
pub fn insert_event(store: &Store, mut event: AgentEvent) -> anyhow::Result<bool> {
    bound_event(&mut event);
    let result = store.insert(&event);
    if let Err(err) = &result {
        crate::health::record_insert_failure(err);
    }
    result
}

pub fn bound_event(event: &mut AgentEvent) {
    if let Some(summary) = &event.summary {
        let too_long = summary.chars().count() > MAX_SUMMARY_CHARS;
        if too_long {
            event.summary = Some(summary.chars().take(MAX_SUMMARY_CHARS).collect());
        }
    }

    let payload_bytes = serde_json::to_vec(&event.payload)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    if payload_bytes > MAX_PAYLOAD_BYTES {
        event.payload = serde_json::json!({ "truncated": true, "bytes": payload_bytes });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracon_core::event::{EventKind, EventSource};

    fn event(summary: &str, payload: serde_json::Value) -> AgentEvent {
        AgentEvent {
            id: None,
            agent: "claude-code".into(),
            session_id: "s1".into(),
            ts: tracon_core::now_iso(),
            kind: EventKind::ToolCall,
            source: EventSource::Hook,
            cwd: None,
            tool_name: Some("Bash".into()),
            summary: Some(summary.into()),
            flag: None,
            payload,
            dedupe_key: None,
        }
    }

    #[test]
    fn long_summary_is_cut_to_300_chars() {
        let mut e = event(&"x".repeat(1000), serde_json::Value::Null);
        bound_event(&mut e);
        assert_eq!(e.summary.as_deref().unwrap().chars().count(), 300);

        let mut short = event("ls", serde_json::Value::Null);
        bound_event(&mut short);
        assert_eq!(short.summary.as_deref(), Some("ls"));
    }

    #[test]
    fn oversized_payload_is_replaced_with_a_note() {
        let big = serde_json::json!({ "output": "y".repeat(MAX_PAYLOAD_BYTES + 1) });
        let mut e = event("ls", big);
        bound_event(&mut e);
        assert_eq!(e.payload["truncated"], true);
        assert!(e.payload["bytes"].as_u64().unwrap() > MAX_PAYLOAD_BYTES as u64);

        let store = Store::open_in_memory().unwrap();
        assert!(insert_event(&store, e).unwrap());
        assert_eq!(store.stats().unwrap().event_count, 1);
    }
}
