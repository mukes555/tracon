use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tracon_core::event::AgentEvent;
use tracon_core::store::{
    CaptureCount, ChangeToken, DayCount, LiveSession, SessionSummary, Stats, Store,
};

struct AppState {
    store: Arc<Store>,
}

/// Every store query runs on a blocking worker: a sync command executes on
/// the app's main thread and freezes the whole window while SQLite works.
async fn run_query<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sessions(state: tauri::State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    let store = state.store.clone();
    run_query(move || store.sessions()).await
}

#[tauri::command]
async fn session_events(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let store = state.store.clone();
    run_query(move || store.events_for_session_lite(&session_id, 750)).await
}

#[tauri::command]
async fn event_payload(
    id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let store = state.store.clone();
    run_query(move || Ok(store.event_payload(id)?.unwrap_or(serde_json::Value::Null))).await
}

#[tauri::command]
async fn change_token(state: tauri::State<'_, AppState>) -> Result<ChangeToken, String> {
    let store = state.store.clone();
    run_query(move || store.change_token()).await
}

#[tauri::command]
async fn stats(state: tauri::State<'_, AppState>) -> Result<Stats, String> {
    let store = state.store.clone();
    run_query(move || store.stats()).await
}

#[tauri::command]
async fn package_events(state: tauri::State<'_, AppState>) -> Result<Vec<AgentEvent>, String> {
    let store = state.store.clone();
    run_query(move || store.package_events(500)).await
}

#[tauri::command]
async fn flagged_events(
    acked: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let store = state.store.clone();
    run_query(move || store.flagged_events(500, acked.unwrap_or(false))).await
}

#[tauri::command]
async fn ack_event(id: i64, acked: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let store = state.store.clone();
    run_query(move || store.set_ack(id, acked)).await
}

#[tauri::command]
async fn search_events(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let store = state.store.clone();
    run_query(move || store.search_events(&query, 60)).await
}

#[tauri::command]
async fn live_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<LiveSession>, String> {
    let store = state.store.clone();
    run_query(move || store.live_sessions_recent(5, 8)).await
}

/// A short recent tail of one session, for the Live page's monitor feeds.
#[tauri::command]
async fn session_tail(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let store = state.store.clone();
    run_query(move || store.events_for_session_lite(&session_id, 12)).await
}

/// Write a session's full event log as pretty JSON into ~/Downloads and
/// return the path. Local file only; nothing leaves the machine.
#[tauri::command]
async fn export_session(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let store = state.store.clone();
    run_query(move || {
        let events = store.events_for_session(&session_id, 100_000)?;
        let json = serde_json::to_vec_pretty(&events)?;

        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let dir = std::path::PathBuf::from(home).join("Downloads");
        std::fs::create_dir_all(&dir)?;

        let short_id: String = session_id.chars().take(8).collect();
        let path = dir.join(format!("tracon-session-{short_id}.json"));
        std::fs::write(&path, json)?;
        Ok(path.display().to_string())
    })
    .await
}

/// Full conversation behind a session, read on demand from the agent's own
/// transcript on disk. Read-only; sessions without a transcript return empty.
/// Transcripts can be hundreds of MB, so the scan runs on a blocking thread;
/// a sync command would freeze the whole window for its duration.
#[tauri::command]
async fn session_thread(
    session_id: String,
) -> Result<Vec<tracon_ingest::thread::ThreadMessage>, String> {
    tauri::async_runtime::spawn_blocking(move || tracon_ingest::thread::read_thread(&session_id))
        .await
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct CaptureStatus {
    claude_dir_found: bool,
    codex_dir_found: bool,
    cursor_found: bool,
    counts: Vec<CaptureCount>,
}

fn cursor_installed() -> bool {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return false;
    };
    let home = std::path::PathBuf::from(home);
    let mac_dir = home.join("Library/Application Support/Cursor");
    let win_dir = home.join("AppData/Roaming/Cursor");
    home.join(".cursor").is_dir() || mac_dir.is_dir() || win_dir.is_dir()
}

#[tauri::command]
async fn capture_status(state: tauri::State<'_, AppState>) -> Result<CaptureStatus, String> {
    use tracon_ingest::tailer;
    let store = state.store.clone();
    run_query(move || {
        Ok(CaptureStatus {
            claude_dir_found: tailer::claude_projects_dir().is_some_and(|d| d.is_dir()),
            codex_dir_found: tailer::codex_sessions_dir().is_some_and(|d| d.is_dir()),
            cursor_found: cursor_installed(),
            counts: store.capture_counts()?,
        })
    })
    .await
}

#[tauri::command]
async fn events_per_day(state: tauri::State<'_, AppState>) -> Result<Vec<DayCount>, String> {
    let store = state.store.clone();
    run_query(move || store.events_per_day(14)).await
}

#[tauri::command]
async fn get_setting(
    key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let store = state.store.clone();
    run_query(move || store.setting(&key)).await
}

#[tauri::command]
async fn set_setting(
    key: String,
    value: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let store = state.store.clone();
    run_query(move || store.set_setting(&key, &value)).await
}

/// Kick off a background import of ALL agent history (no time window).
/// Returns immediately; events stream into the store as files process.
#[tauri::command]
fn import_full_history(state: tauri::State<'_, AppState>) -> Result<String, String> {
    use tracon_ingest::tailer;
    let store = state.store.clone();
    std::thread::spawn(move || {
        if let Some(dir) = tailer::claude_projects_dir() {
            tailer::import_full_tree(&store, &dir, tailer::claude_parser);
        }
        if let Some(dir) = tailer::codex_sessions_dir() {
            tailer::import_full_tree(&store, &dir, tailer::codex_parser);
        }
        let _ = tracon_ingest::flags::backfill_flags(&store);
    });
    Ok("started".into())
}

#[tauri::command]
fn data_dir(app: tauri::AppHandle) -> Result<String, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.display().to_string())
        .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let store = open_store(app)?;
            app.manage(AppState {
                store: store.clone(),
            });
            spawn_ingest_server(store.clone());
            spawn_spool_drainer(store.clone());
            spawn_log_tailers(store.clone());
            tracon_ingest::apps::spawn_app_watcher(store.clone());
            spawn_flag_backfill(store.clone());
            spawn_flag_notifier(app.handle().clone(), store.clone());
            tauri::async_runtime::spawn(tracon_ingest::intel::run_worker(store.clone()));
            tauri::async_runtime::spawn(tracon_ingest::retention::run_worker(store.clone()));
            build_tray(app, store)?;
            Ok(())
        })
        // Closing the window must not stop the recorder: hide to tray instead.
        // Quit lives in the tray menu.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            sessions,
            session_events,
            change_token,
            stats,
            package_events,
            flagged_events,
            ack_event,
            live_sessions,
            session_tail,
            search_events,
            session_thread,
            capture_status,
            export_session,
            events_per_day,
            event_payload,
            get_setting,
            set_setting,
            import_full_history,
            data_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tracon");
}

fn open_store(app: &tauri::App) -> anyhow::Result<Arc<Store>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    Ok(Arc::new(Store::open(&data_dir.join("tracon.db"))?))
}

fn spawn_ingest_server(store: Arc<Store>) {
    tauri::async_runtime::spawn(async move {
        // A failed bind (port in use) must not take the app down; the UI still
        // serves historical data and the user gets pointed at the port setting.
        if let Err(err) = tracon_ingest::serve(store, tracon_ingest::DEFAULT_PORT).await {
            eprintln!("tracon: ingest server not running: {err}");
        }
    });
}

/// Backfill events the plugin spooled while the app was closed, then keep
/// draining periodically so long-lived sessions don't wait for a restart.
fn spawn_spool_drainer(store: Arc<Store>) {
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
fn spawn_log_tailers(store: Arc<Store>) {
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
fn spawn_flag_notifier(app: tauri::AppHandle, store: Arc<Store>) {
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
fn spawn_flag_backfill(store: Arc<Store>) {
    std::thread::spawn(move || {
        let _ = tracon_ingest::flags::backfill_flags(&store);
    });
}

/// The tray is the at-a-glance surface: today's numbers, open flags, and a
/// pause switch, refreshed every 30s (menu mutation must happen on the main
/// thread on macOS, hence run_on_main_thread).
fn build_tray(app: &tauri::App, store: Arc<Store>) -> tauri::Result<()> {
    let menu = tray_menu(app.handle(), &store)?;
    let menu_store = store.clone();

    TrayIconBuilder::with_id("tracon-tray")
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" | "summary" | "flags" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "pause" => {
                let now_paused = !menu_store.capture_paused();
                let _ = menu_store
                    .set_setting("capture_paused", if now_paused { "true" } else { "false" });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let h = handle.clone();
            let s = store.clone();
            let _ = handle.run_on_main_thread(move || {
                if let Some(tray) = h.tray_by_id("tracon-tray") {
                    if let Ok(menu) = tray_menu(&h, &s) {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
            });
        }
    });
    Ok(())
}

fn tray_menu(app: &tauri::AppHandle, store: &Store) -> tauri::Result<Menu<tauri::Wry>> {
    let (summary_text, flags_text) = match store.stats() {
        Ok(stats) => (
            format!(
                "Today: {} sessions · {} commands",
                stats.sessions_today, stats.commands_today
            ),
            format!("Open flags: {}", stats.flagged_count),
        ),
        Err(_) => ("Tracon".into(), "Open flags: -".into()),
    };
    let summary = MenuItem::with_id(app, "summary", summary_text, false, None::<&str>)?;
    let flags = MenuItem::with_id(app, "flags", flags_text, true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Tracon", true, None::<&str>)?;
    let pause = tauri::menu::CheckMenuItem::with_id(
        app,
        "pause",
        "Pause capture",
        true,
        store.capture_paused(),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Tracon", true, None::<&str>)?;
    Menu::with_items(app, &[&summary, &flags, &open, &pause, &quit])
}
