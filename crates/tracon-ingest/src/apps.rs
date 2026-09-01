//! Watches application directories for newly installed apps (Figma, anything),
//! regardless of how they got there: agent command, DMG drag, or installer.
//! Read-only observation; events land in the Packages view under the
//! "system" agent since directory watching alone cannot attribute a source.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;

use notify::{EventKind as NotifyKind, RecursiveMode, Watcher};
use tracon_core::event::{AgentEvent, EventKind, EventSource};
use tracon_core::now_iso;
use tracon_core::store::Store;

pub const AGENT_NAME: &str = "system";
const SESSION_ID: &str = "system-apps";

fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        dirs.push(home.join("Applications"));
        dirs.push(home.join("AppData").join("Local").join("Programs"));
    }
    dirs.push(PathBuf::from("C:\\Program Files"));
    dirs.retain(|d| d.is_dir());
    dirs
}

pub fn spawn_app_watcher(store: Arc<Store>) {
    let dirs = app_dirs();
    if dirs.is_empty() {
        return;
    }
    std::thread::Builder::new()
        .name("tracon-app-watcher".into())
        .spawn(move || run_watcher(store, dirs))
        .ok();
}

fn run_watcher(store: Arc<Store>, dirs: Vec<PathBuf>) {
    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(_) => return,
    };
    for dir in &dirs {
        // Non-recursive: only top-level additions mean "an app was installed";
        // recursing into app bundles would be pure noise.
        let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
    }

    for result in rx {
        let Ok(event) = result else { continue };
        if !matches!(event.kind, NotifyKind::Create(_)) {
            continue;
        }
        for path in event.paths {
            if let Some(mut record) = app_install_event(&path) {
                enrich_with_active_agents(&store, &mut record);
                let _ = store.insert(&record);
            }
        }
    }
}

/// Directory watching can't prove who installed an app, but it can say who
/// was flying at the time: if any agent produced events in the last few
/// minutes, the install is annotated with them. A correlation, not a verdict.
pub fn enrich_with_active_agents(store: &Store, event: &mut AgentEvent) {
    let cutoff = time::OffsetDateTime::now_utc() - time::Duration::minutes(3);
    let Ok(cutoff) = cutoff.format(&time::format_description::well_known::Rfc3339) else {
        return;
    };
    let Ok(agents) = store.agents_active_since(&cutoff) else {
        return;
    };
    if agents.is_empty() {
        return;
    }
    if let Some(summary) = &event.summary {
        event.summary = Some(format!("{summary} (during {} session)", agents.join(" + ")));
    }
    if let Some(map) = event.payload.as_object_mut() {
        map.insert("active_agents".into(), serde_json::json!(agents));
    }
}

/// A new top-level .app bundle (macOS) or program directory (Windows) counts
/// as an application install. Hidden and partial-download entries are skipped.
pub fn app_install_event(path: &Path) -> Option<AgentEvent> {
    let name = path.file_name()?.to_str()?;
    if name.starts_with('.') || name.ends_with(".download") || name.ends_with(".part") {
        return None;
    }
    let is_mac_app = name.ends_with(".app");
    let is_windows_program = cfg!(windows) && path.is_dir();
    if !is_mac_app && !is_windows_program {
        return None;
    }

    let display = name.trim_end_matches(".app").to_string();
    let ts = now_iso();
    let day: String = ts.chars().take(10).collect();
    Some(AgentEvent {
        id: None,
        agent: AGENT_NAME.into(),
        session_id: SESSION_ID.into(),
        ts,
        kind: EventKind::PackageInstall,
        source: EventSource::Process,
        cwd: path.parent().map(|p| p.display().to_string()),
        tool_name: Some("app".into()),
        summary: Some(format!("Application installed: {display}")),
        flag: None,
        payload: serde_json::json!({ "path": path.display().to_string() }),
        dedupe_key: Some(format!("system|app|{}|{day}", path.display())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_app_bundle_becomes_install_event() {
        let event = app_install_event(Path::new("/Applications/Figma.app")).unwrap();
        assert_eq!(event.kind, EventKind::PackageInstall);
        assert_eq!(event.agent, "system");
        assert_eq!(
            event.summary.as_deref(),
            Some("Application installed: Figma")
        );
    }

    #[test]
    fn app_install_gets_attributed_to_active_agents() {
        let store = Store::open_in_memory().unwrap();
        store
            .insert(&AgentEvent {
                id: None,
                agent: "claude-code".into(),
                session_id: "s1".into(),
                ts: now_iso(),
                kind: EventKind::ToolCall,
                source: EventSource::Hook,
                cwd: None,
                tool_name: Some("Bash".into()),
                summary: Some("ls".into()),
                flag: None,
                payload: serde_json::Value::Null,
                dedupe_key: None,
            })
            .unwrap();

        let mut event = app_install_event(Path::new("/Applications/Figma.app")).unwrap();
        enrich_with_active_agents(&store, &mut event);
        assert!(
            event
                .summary
                .as_deref()
                .unwrap()
                .contains("during claude-code session"),
            "summary was: {:?}",
            event.summary
        );
        assert_eq!(event.payload["active_agents"][0], "claude-code");
    }

    #[test]
    fn hidden_and_partial_entries_are_ignored() {
        assert!(app_install_event(Path::new("/Applications/.DS_Store")).is_none());
        assert!(app_install_event(Path::new("/Applications/Figma.app.download")).is_none());
        // A plain file that is not an app bundle is ignored on macOS.
        assert!(app_install_event(Path::new("/Applications/notes.txt")).is_none());
    }
}
