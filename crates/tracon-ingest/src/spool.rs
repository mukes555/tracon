use std::path::{Path, PathBuf};

use serde_json::Value;
use tracon_core::store::Store;

/// Where the plugin's async command hook appends events while the app is closed.
pub fn default_spool_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".tracon").join("spool.ndjson"))
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
    let content = std::fs::read_to_string(&work)?;

    let mut inserted = 0;
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        for event in tracon_adapters::events_from_any_hook_payload(&value) {
            if store.insert(&event).unwrap_or(false) {
                inserted += 1;
            }
        }
    }

    std::fs::remove_file(&work)?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_lines_once_and_removes_file() {
        let store = Store::open_in_memory().unwrap();
        let dir = std::env::temp_dir().join(format!("tracon-spool-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let spool = dir.join("spool.ndjson");

        let line = serde_json::json!({
            "session_id": "s1",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_use_id": "t1",
            "tool_input": {"command": "cargo build"}
        })
        .to_string();
        std::fs::write(&spool, format!("{line}\nnot-json\n{line}\n")).unwrap();

        let inserted = drain_spool(&store, &spool).unwrap();
        assert_eq!(inserted, 1);
        assert!(!spool.exists());
        assert_eq!(store.stats().unwrap().event_count, 1);

        // A second drain with no file is a clean no-op.
        assert_eq!(drain_spool(&store, &spool).unwrap(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
