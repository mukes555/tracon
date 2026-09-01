//! Tails agent log trees: Claude Code transcripts and Codex CLI rollouts.
//!
//! HARD RULE: this module only ever READS from agent directories. The user's
//! real agent setups must never be modified; offsets and events are written
//! exclusively to Tracon's own database.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use notify::{RecursiveMode, Watcher};
use tracon_core::event::AgentEvent;
use tracon_core::store::Store;

/// Files older than this are skipped during the startup scan so a machine with
/// months of real agent history doesn't get churned through on first launch.
const BACKFILL_WINDOW: Duration = Duration::from_secs(3 * 24 * 60 * 60);
const MAX_SCAN_DEPTH: usize = 5;

/// Turns one log line (plus its file path, for filename-derived context)
/// into normalized events.
pub type LineParser = fn(&str, &Path) -> Vec<AgentEvent>;

pub fn claude_parser(line: &str, _path: &Path) -> Vec<AgentEvent> {
    tracon_adapters::claude_transcript::parse_transcript_line(line)
}

pub fn codex_parser(line: &str, path: &Path) -> Vec<AgentEvent> {
    let session_id = tracon_adapters::codex::session_id_from_path(path);
    tracon_adapters::codex::parse_rollout_line(line, &session_id)
}

pub fn claude_projects_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(".claude").join("projects"))
}

pub fn codex_sessions_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(".codex").join("sessions"))
}

fn home_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home))
}

/// Watch a log tree on a dedicated thread: backfill recent files, then
/// process appends as they happen. All errors are soft; tailing is a
/// best-effort supplement to the hook stream.
pub fn spawn_log_tailer(store: Arc<Store>, root: PathBuf, parser: LineParser) {
    std::thread::Builder::new()
        .name("tracon-log-tailer".into())
        .spawn(move || run_tailer(store, root, parser))
        .ok();
}

fn run_tailer(store: Arc<Store>, root: PathBuf, parser: LineParser) {
    if !root.is_dir() {
        return;
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(err) => {
            eprintln!("tracon: log watcher unavailable: {err}");
            return;
        }
    };
    if let Err(err) = watcher.watch(&root, RecursiveMode::Recursive) {
        eprintln!("tracon: cannot watch {}: {err}", root.display());
        return;
    }

    scan_dir(&store, &root, parser, 0, false);

    for result in rx {
        let Ok(event) = result else { continue };
        for path in event.paths {
            if is_log_file(&path) {
                process_file(&store, &path, parser);
            }
        }
    }
}

fn is_log_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
}

/// One-shot import of an entire log tree, ignoring the backfill window.
/// Explicit user action ("import full history"); offsets still dedupe reruns.
pub fn import_full_tree(store: &Store, root: &Path, parser: LineParser) {
    scan_dir(store, root, parser, 0, true);
}

/// Recursive backfill of recently modified files; Codex nests rollouts under
/// YYYY/MM/DD, Claude keeps one directory per project.
fn scan_dir(store: &Store, dir: &Path, parser: LineParser, depth: usize, all: bool) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(store, &path, parser, depth + 1, all);
        } else if is_log_file(&path) && (all || modified_recently(&path)) {
            process_file(store, &path, parser);
        }
    }
}

fn modified_recently(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age <= BACKFILL_WINDOW)
        .unwrap_or(true)
}

/// Read everything new since the stored offset, parse complete lines, and
/// advance the offset only past the last full line so a partially written
/// line is picked up whole on the next change.
pub fn process_file(store: &Store, path: &Path, parser: LineParser) {
    if store.capture_paused() {
        return;
    }
    let path_key = path.to_string_lossy().to_string();
    let mut offset = store.tail_offset(&path_key).unwrap_or(0);

    let Ok(meta) = path.metadata() else { return };
    if meta.len() < offset {
        // The file was truncated or replaced; start over.
        offset = 0;
    }
    if meta.len() == offset {
        return;
    }

    let Ok(mut file) = File::open(path) else {
        return;
    };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return;
    }
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return;
    }

    let consumed = match buf.rfind('\n') {
        Some(idx) => idx + 1,
        None => return,
    };
    for line in buf[..consumed].lines() {
        for event in parser(line, path) {
            let _ = store.insert(&event);
        }
    }
    let _ = store.set_tail_offset(&path_key, offset + consumed as u64);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tracon-tailer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tool_use_line(session: &str, id: &str, command: &str) -> String {
        serde_json::json!({
            "type": "assistant",
            "sessionId": session,
            "timestamp": "2026-08-29T12:00:00Z",
            "message": {"content": [
                {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": command}}
            ]}
        })
        .to_string()
    }

    #[test]
    fn processes_appends_incrementally_without_reprocessing() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir();
        let file = dir.join("session.jsonl");

        std::fs::write(&file, format!("{}\n", tool_use_line("s1", "t1", "ls"))).unwrap();
        process_file(&store, &file, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 1);

        // Reprocessing with no new content is a no-op.
        process_file(&store, &file, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 1);

        // Appending only processes the new line; a partial line waits.
        let mut content = std::fs::read_to_string(&file).unwrap();
        content.push_str(&format!(
            "{}\n",
            tool_use_line("s1", "t2", "npm install left-pad")
        ));
        content.push_str("{\"type\":\"assistant\",\"sessionId\":\"s1\"");
        std::fs::write(&file, &content).unwrap();
        process_file(&store, &file, claude_parser);

        // t2 yields a tool_call plus a package_install.
        assert_eq!(store.stats().unwrap().event_count, 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn codex_rollouts_in_nested_dirs_are_backfilled() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir();
        let nested = dir.join("2026").join("08").join("29");
        std::fs::create_dir_all(&nested).unwrap();
        let file =
            nested.join("rollout-2026-08-29T10-00-00-8f14e45f-ceea-4a67-a1b2-c3d4e5f60718.jsonl");
        let line = serde_json::json!({
            "type": "response_item",
            "payload": {"type": "function_call", "name": "shell", "call_id": "c1",
                        "arguments": "{\"command\": [\"bash\", \"-lc\", \"cargo add serde\"]}"}
        })
        .to_string();
        std::fs::write(&file, format!("{line}\n")).unwrap();

        scan_dir(&store, &dir, codex_parser, 0, false);
        let stats = store.stats().unwrap();
        assert_eq!(stats.event_count, 2);
        assert_eq!(stats.package_count, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
