//! The menu bar icon and its menu.
use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tracon_core::store::Store;

/// The tray is the at-a-glance surface: today's numbers, open flags, and a
/// pause switch, refreshed every 30s (menu mutation must happen on the main
/// thread on macOS, hence run_on_main_thread).
pub(crate) fn build_tray(app: &tauri::App, store: Arc<Store>) -> tauri::Result<()> {
    let menu = tray_menu(app.handle(), &store)?;
    let menu_store = store.clone();

    // A monochrome template icon: macOS tints it for light and dark menu
    // bars, where the full-color app icon just reads as a dark smudge.
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    TrayIconBuilder::with_id("tracon-tray")
        .icon(tray_icon)
        .icon_as_template(true)
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
            "update" => {
                use tauri_plugin_opener::OpenerExt;
                let url = menu_store
                    .setting("update_url")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "https://github.com/mukes555/tracon/releases/latest".into());
                let _ = app.opener().open_url(url, None::<&str>);
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
    let current = app.package_info().version.to_string();
    let update = crate::updates::status(store, &current);
    if !update.available {
        return Menu::with_items(app, &[&summary, &flags, &open, &pause, &quit]);
    }
    let latest = update.latest.unwrap_or_default();
    let update_item = MenuItem::with_id(
        app,
        "update",
        format!("Update available: {latest}"),
        true,
        None::<&str>,
    )?;
    Menu::with_items(app, &[&summary, &flags, &update_item, &open, &pause, &quit])
}
