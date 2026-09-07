use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracon_core::event::AgentEvent;
use tracon_core::store::Store;

use crate::sink::insert_event;

/// Where the plugin's async command hook appends events while the app is closed.
pub fn default_spool_path() -> Option<PathBuf> {
    Some(crate::auth::tracon_dir()?.join("spool.ndjson"))
}

/// Drain the spool file into the store. Returns how many events were newly inserted.
///
/// The file is renamed before reading so hook invocations that start during the
/// drain append to a fresh spool instead of the one being consumed. A hook that
/// already holds the old file open can in rare cases lose its line; the spool
/// only ever supplements the HTTP path, so that loss is accepted for now.
pub fn drain_spool(store: &Store, path: &Path) -> anyhow::Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let work = path.with_extension("draining");
    std::fs::rename(path, &work)?;
    let reader = BufReader::new(File::open(&work)?);

    let mut inserted = 0;
    for line in reader.split(b'\n') {
        let Ok(line) = line else { break };
        let text = String::from_utf8_lossy(&line);
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        for event in spooled_events(&value) {
            if insert_event(store, event).unwrap_or(false) {
                inserted += 1;
            }
        }
    }

    std::fs::remove_file(&work)?;
    Ok(inserted)
}

/// A spool line is either a raw hook payload (older plugin builds) or the
/// current hook's {"ts": ..., "payload": ...} envelope. The envelope's
/// timestamp replaces the adapter's "now", so a command run while the app was
/// closed keeps the time it actually ran instead of the drain time.
fn spooled_events(line: &Value) -> Vec<AgentEvent> {
    let has_payload_object = line.get("payload").is_some_and(Value::is_object);
    let is_envelope = line.get("ts").is_some() && has_payload_object;
    let payload = if is_envelope { &line["payload"] } else { line };
    let recorded_ts = if is_envelope {
        line.get("ts")
            .and_then(Value::as_str)
            .filter(|ts| OffsetDateTime::parse(ts, &Rfc3339).is_ok())
    } else {
        None
    };

    let mut events = tracon_adapters::events_from_any_hook_payload(payload);
    if let Some(ts) = recorded_ts {
        for event in &mut events {
            event.ts = ts.to_string();
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    fn hook_payload(tool_use_id: &str) -> Value {
        serde_json::json!({
            "session_id": "s1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": tool_use_id,
            "tool_input": {"command": "cargo build"}
        })
    }

    #[test]
    fn drains_lines_once_and_removes_file() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir("spool");
        let spool = dir.join("spool.ndjson");

        let line = hook_payload("t1").to_string();
        std::fs::write(&spool, format!("{line}\nnot-json\n{line}\n")).unwrap();

        let inserted = drain_spool(&store, &spool).unwrap();
        assert_eq!(inserted, 1);
        assert!(!spool.exists());
        assert_eq!(store.stats().unwrap().event_count, 1);

        // A second drain with no file is a clean no-op.
        assert_eq!(drain_spool(&store, &spool).unwrap(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wrapped_line_keeps_the_recorded_timestamp() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir("spool-ts");
        let spool = dir.join("spool.ndjson");

        let wrapped = serde_json::json!({
            "ts": "2026-08-30T08:15:00Z",
            "payload": hook_payload("t1")
        });
        let bad_ts = serde_json::json!({ "ts": "yesterday", "payload": hook_payload("t2") });
        std::fs::write(&spool, format!("{wrapped}\n{bad_ts}\n")).unwrap();

        assert_eq!(drain_spool(&store, &spool).unwrap(), 2);
        let events = store.events_for_session("s1", 10).unwrap();
        let recorded = events
            .iter()
            .find(|e| e.payload["tool_use_id"] == "t1")
            .unwrap();
        assert_eq!(recorded.ts, "2026-08-30T08:15:00.000Z");
        // An unparseable ts falls back to the adapter's own timestamp.
        let fallback = events
            .iter()
            .find(|e| e.payload["tool_use_id"] == "t2")
            .unwrap();
        assert_ne!(fallback.ts, "yesterday");
        std::fs::remove_dir_all(&dir).ok();
    }
}
