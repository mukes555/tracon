use std::sync::Arc;

use tauri::Manager;

mod tray;
mod workers;
use tracon_core::event::AgentEvent;
use tracon_core::store::{CaptureCount, DayCount, LiveSession, SessionSummary, Stats, Store};

struct AppState {
    store: Arc<Store>,
    log_path: std::path::PathBuf,
}

/// The core change token plus the capture switch, so the UI learns about a
/// pause from the same 3s poll it already runs.
#[derive(serde::Serialize)]
struct ChangeToken {
    #[serde(flatten)]
    core: tracon_core::store::ChangeToken,
    paused: bool,
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
    run_query(move || {
        Ok(ChangeToken {
            core: store.change_token()?,
            paused: store.capture_paused(),
        })
    })
    .await
}

#[tauri::command]
async fn capture_paused(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let store = state.store.clone();
    run_query(move || Ok(store.capture_paused())).await
}

#[tauri::command]
async fn set_capture_paused(paused: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let store = state.store.clone();
    run_query(move || store.set_setting("capture_paused", if paused { "true" } else { "false" }))
        .await
}

/// Delete every recorded event. The UI asks twice before calling this.
#[tauri::command]
async fn purge_all(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    let store = state.store.clone();
    run_query(move || {
        let removed = store.stats()?.event_count;
        store.purge_all()?;
        store.compact()?;
        Ok(removed)
    })
    .await
}

#[tauri::command]
async fn purge_session(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let store = state.store.clone();
    run_query(move || store.purge_session(&session_id)).await
}

#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
fn log_path(state: tauri::State<'_, AppState>) -> String {
    state.log_path.display().to_string()
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
async fn ack_events(
    ids: Vec<i64>,
    acked: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let store = state.store.clone();
    run_query(move || store.set_ack(&ids, acked)).await
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
    run_query(move || store.live_sessions_recent(8)).await
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
    gemini_found: bool,
    insert_failures: u64,
    last_error: Option<String>,
    counts: Vec<CaptureCount>,
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

fn gemini_installed() -> bool {
    home_dir().is_some_and(|h| h.join(".gemini").is_dir())
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
            gemini_found: gemini_installed(),
            insert_failures: tracon_ingest::health().insert_failures,
            last_error: tracon_ingest::health().last_error,
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
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Ok("already running".into());
    }
    let store = state.store.clone();
    std::thread::spawn(move || {
        struct Done;
        impl Drop for Done {
            fn drop(&mut self) {
                RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _done = Done;
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
        // A rotating file in the app log dir, revealed from Settings, so a
        // user can send something when capture goes wrong.
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: None },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let store = open_store(app)?;
            // The log plugin names the file after the app when no name is given.
            let log_name = format!("{}.log", app.package_info().name);
            let log_path = app.path().app_log_dir()?.join(log_name);
            app.manage(AppState {
                store: store.clone(),
                log_path,
            });
            let token = workers::ingest_token()?;
            workers::spawn_ingest_server(store.clone(), token);
            workers::spawn_spool_drainer(store.clone());
            workers::spawn_log_tailers(store.clone());
            tracon_ingest::apps::spawn_app_watcher(store.clone());
            workers::spawn_flag_backfill(store.clone());
            workers::spawn_flag_notifier(app.handle().clone(), store.clone());
            tauri::async_runtime::spawn(tracon_ingest::intel::run_worker(store.clone()));
            tauri::async_runtime::spawn(tracon_ingest::retention::run_worker(store.clone()));
            tray::build_tray(app, store)?;
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
            ack_events,
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
            data_dir,
            capture_paused,
            set_capture_paused,
            purge_all,
            purge_session,
            app_version,
            log_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tracon");
}

fn open_store(app: &tauri::App) -> anyhow::Result<Arc<Store>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    Ok(Arc::new(Store::open(&data_dir.join("tracon.db"))?))
}
