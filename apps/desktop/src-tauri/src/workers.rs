//! Background work that runs for the life of the app: the ingest server,
//! spool drain, log tailers, flag notifier, and flag backfill.
use std::sync::Arc;

use tracon_core::store::Store;

/// The per-install secret hooks must present. Lives beside the spool in
/// ~/.tracon so the hook configs can read it with one cat.
pub(crate) fn ingest_token() -> anyhow::Result<String> {
    let dir = tracon_ingest::tracon_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    std::fs::create_dir_all(&dir)?;
    tracon_ingest::ensure_token(&dir)
}

pub(crate) fn spawn_ingest_server(store: Arc<Store>, token: String) {
    tauri::async_runtime::spawn(async move {
        // A failed bind (port in use) must not take the app down; the UI still
        // serves historical data and the log says why capture is off.
        if let Err(err) = tracon_ingest::serve(store, tracon_ingest::DEFAULT_PORT, token).await {
            log::error!("ingest server not running: {err}");
        }
    });
}

/// Backfill events the plugin spooled while the app was closed, then keep
/// draining periodically so long-lived sessions don't wait for a restart.
pub(crate) fn spawn_spool_drainer(store: Arc<Store>) {
    let Some(spool_path) = tracon_ingest::spool::default_spool_path() else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        loop {
            let _ = tracon_ingest::spool::drain_spool(&store, &spool_path);
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

/// Read-only tailing of agent log trees for backfill and cross-checks.
/// Tracon never writes to ~/.claude or ~/.codex; offsets live in its own DB.
pub(crate) fn spawn_log_tailers(store: Arc<Store>) {
    use tracon_ingest::tailer;
    if let Some(dir) = tailer::claude_projects_dir() {
        tailer::spawn_log_tailer(store.clone(), dir, tailer::claude_parser);
    }
    if let Some(dir) = tailer::codex_sessions_dir() {
        tailer::spawn_log_tailer(store, dir, tailer::codex_parser);
    }
}

/// OS notification when an agent does something flag-worthy: the whole point
/// of a watchdog is telling you while you're away. Baselines at the current
/// max id so history never renotifies; the toggle lives in Settings.
pub(crate) fn spawn_flag_notifier(app: tauri::AppHandle, store: Arc<Store>) {
    use tauri_plugin_notification::NotificationExt;

    tauri::async_runtime::spawn(async move {
        let mut last_id = store.max_event_id().unwrap_or(0);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let enabled = store
                .setting("notify_flags")
                .ok()
                .flatten()
                .map(|v| v != "false")
                .unwrap_or(true);
            if !enabled {
                // Keep the baseline moving so re-enabling doesn't dump backlog.
                last_id = store.max_event_id().unwrap_or(last_id);
                continue;
            }
            let Ok(fresh) = store.flagged_events_after(last_id, 20) else {
                continue;
            };
            if fresh.is_empty() {
                // Nothing flagged since the cursor: move it to the newest row
                // so the next scan does not walk the same rows again.
                last_id = store.max_event_id().unwrap_or(last_id);
                continue;
            }
            last_id = fresh.iter().filter_map(|e| e.id).max().unwrap_or(last_id);

            if fresh.len() > 3 {
                let _ = app
                    .notification()
                    .builder()
                    .title("Tracon: multiple actions flagged")
                    .body(format!(
                        "{} flagged agent actions just recorded",
                        fresh.len()
                    ))
                    .show();
                continue;
            }
            for event in fresh {
                let flag = event.flag.unwrap_or_else(|| "flagged".into());
                let what = event.summary.unwrap_or_default();
                let body: String = format!("{flag}: {what}").chars().take(160).collect();
                let _ = app
                    .notification()
                    .builder()
                    .title("Tracon flagged an agent action")
                    .body(body)
                    .show();
            }
        }
    });
}

/// One pass over events recorded before danger flags existed.
pub(crate) fn spawn_flag_backfill(store: Arc<Store>) {
    std::thread::spawn(move || {
        let _ = tracon_ingest::flags::backfill_flags(&store);
    });
}
