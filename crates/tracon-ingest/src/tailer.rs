//! Tails agent log trees: Claude Code transcripts and Codex CLI rollouts.
//!
//! HARD RULE: this module only ever READS from agent directories. The user's
//! real agent setups must never be modified; offsets and events are written
//! exclusively to Tracon's own database.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tracon_core::event::AgentEvent;
use tracon_core::store::Store;

pub use crate::tail_read::process_file;

/// Files older than this are skipped during the startup scan so a machine with
/// months of real agent history doesn't get churned through on first launch.
const BACKFILL_WINDOW: Duration = Duration::from_secs(3 * 24 * 60 * 60);
const MAX_SCAN_DEPTH: usize = 5;
/// Bounds memory when the watcher outpaces the parser: notify blocks on a
/// full channel instead of queueing events without limit.
const WATCH_QUEUE: usize = 4096;
const RESCAN_INTERVAL: Duration = Duration::from_secs(5);

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
    let Ok(root) = root.canonicalize() else {
        return;
    };
    if !root.is_dir() {
        return;
    }

    let (tx, rx) = mpsc::sync_channel(WATCH_QUEUE);
    let watcher = start_watcher(&root, tx);
    scan_dir(&store, &root, parser, false);

    // Without a watcher (inotify limits, unsupported filesystem) the tree is
    // simply rescanned on a timer: slower, but still captures everything.
    let Some(_watcher) = watcher else {
        loop {
            std::thread::sleep(RESCAN_INTERVAL);
            scan_dir(&store, &root, parser, false);
        }
    };

    for result in rx {
        let Ok(event) = result else { continue };
        for path in event.paths {
            if is_tailable_log(&path, &root) {
                process_file_guarded(&store, &path, parser);
            }
        }
    }
}

fn start_watcher(
    root: &Path,
    tx: mpsc::SyncSender<notify::Result<notify::Event>>,
) -> Option<RecommendedWatcher> {
    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(err) => {
            eprintln!("tracon: log watcher unavailable, polling instead: {err}");
            return None;
        }
    };
    if let Err(err) = watcher.watch(root, RecursiveMode::Recursive) {
        eprintln!(
            "tracon: cannot watch {}, polling instead: {err}",
            root.display()
        );
        return None;
    }
    Some(watcher)
}

/// A parser bug on one weird line must not take the whole tailer thread down.
fn process_file_guarded(store: &Store, path: &Path, parser: LineParser) {
    let outcome = catch_unwind(AssertUnwindSafe(|| process_file(store, path, parser)));
    if outcome.is_err() {
        eprintln!("tracon: parser panicked on {}, skipping", path.display());
    }
}

fn is_log_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
}

/// Only regular .jsonl files whose real location is inside the watched tree.
/// Symlinks are refused outright: a link planted in an agent directory must
/// not turn the tailer into a reader of arbitrary files.
fn is_tailable_log(path: &Path, canonical_root: &Path) -> bool {
    if !is_log_file(path) {
        return false;
    }
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    path.canonicalize()
        .map(|real| real.starts_with(canonical_root))
        .unwrap_or(false)
}

/// One-shot import of an entire log tree, ignoring the backfill window.
/// Explicit user action ("import full history"); offsets still dedupe reruns.
pub fn import_full_tree(store: &Store, root: &Path, parser: LineParser) {
    let Ok(root) = root.canonicalize() else {
        return;
    };
    scan_dir(store, &root, parser, true);
}

/// Recursive backfill of recently modified files; Codex nests rollouts under
/// YYYY/MM/DD, Claude keeps one directory per project. `root` must already be
/// canonical so the containment check is meaningful.
fn scan_dir(store: &Store, root: &Path, parser: LineParser, all: bool) {
    scan_subtree(store, root, root, parser, 0, all);
}

fn scan_subtree(
    store: &Store,
    root: &Path,
    dir: &Path,
    parser: LineParser,
    depth: usize,
    all: bool,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        // file_type() does not follow symlinks, which is exactly the point.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            scan_subtree(store, root, &path, parser, depth + 1, all);
            continue;
        }
        let wanted = all || modified_recently(&path);
        if wanted && is_tailable_log(&path, root) {
            process_file_guarded(store, &path, parser);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    fn tool_use_line(id: &str, command: &str) -> String {
        serde_json::json!({
            "type": "assistant",
            "sessionId": "s1",
            "timestamp": "2026-08-29T12:00:00Z",
            "message": {"content": [
                {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": command}}
            ]}
        })
        .to_string()
    }

    #[test]
    fn codex_rollouts_in_nested_dirs_are_backfilled() {
        let store = Store::open_in_memory().unwrap();
        let dir = temp_dir("tail-codex");
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

        import_full_tree(&store, &dir, codex_parser);
        let stats = store.stats().unwrap();
        assert_eq!(stats.event_count, 2);
        assert_eq!(stats.package_count, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_and_dirs_are_skipped() {
        use std::os::unix::fs::symlink;

        let store = Store::open_in_memory().unwrap();
        let outside = temp_dir("tail-outside");
        let root = temp_dir("tail-root");

        let real = outside.join("secret.jsonl");
        std::fs::write(&real, format!("{}\n", tool_use_line("t1", "ls"))).unwrap();
        symlink(&real, root.join("link.jsonl")).unwrap();
        symlink(&outside, root.join("linked-dir")).unwrap();

        import_full_tree(&store, &root, claude_parser);
        assert_eq!(store.stats().unwrap().event_count, 0);

        // A watcher event naming the symlink is refused too.
        let canonical_root = root.canonicalize().unwrap();
        assert!(!is_tailable_log(&root.join("link.jsonl"), &canonical_root));
        assert!(is_tailable_log(&real, &outside.canonicalize().unwrap()));

        std::fs::remove_dir_all(&outside).ok();
        std::fs::remove_dir_all(&root).ok();
    }
}
