use serde_json::Value;
use tracon_core::store::Store;

/// Assess danger flags for Bash events recorded before flag support existed.
/// Runs at startup; safe commands stay NULL and are cheaply re-scanned, which
/// keeps this stateless. Returns how many events got flagged.
pub fn backfill_flags(store: &Store) -> anyhow::Result<usize> {
    let candidates = store.unassessed_bash_events(100_000)?;
    let mut flagged = 0;
    for (id, payload_json) in candidates {
        let Ok(payload) = serde_json::from_str::<Value>(&payload_json) else {
            continue;
        };
        let Some(command) = extract_command(&payload) else {
            continue;
        };
        if let Some(flag) = tracon_adapters::danger::assess_command(command) {
            store.set_flag(id, &flag)?;
            flagged += 1;
        }
    }
    Ok(flagged)
}

/// Hook payloads keep the command under tool_input; transcript tool_use
/// blocks keep it under input.
fn extract_command(payload: &Value) -> Option<&str> {
    payload
        .get("tool_input")
        .or_else(|| payload.get("input"))?
        .get("command")?
        .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracon_core::event::{AgentEvent, EventKind, EventSource};
    use tracon_core::now_iso;

    #[test]
    fn backfills_flags_for_old_bash_events() {
        let store = Store::open_in_memory().unwrap();
        let mut event = AgentEvent {
            id: None,
            agent: "claude-code".into(),
            session_id: "s1".into(),
            ts: now_iso(),
            kind: EventKind::ToolCall,
            source: EventSource::LogTail,
            cwd: None,
            tool_name: Some("Bash".into()),
            summary: Some("rm -rf ~/".into()),
            flag: None,
            payload: serde_json::json!({"input": {"command": "rm -rf ~/"}}),
            dedupe_key: None,
        };
        store.insert(&event).unwrap();
        event.payload = serde_json::json!({"input": {"command": "ls"}});
        event.summary = Some("ls".into());
        store.insert(&event).unwrap();

        assert_eq!(backfill_flags(&store).unwrap(), 1);
        assert_eq!(store.stats().unwrap().flagged_count, 1);
        // Second run finds nothing new to flag.
        assert_eq!(backfill_flags(&store).unwrap(), 0);
    }
}
