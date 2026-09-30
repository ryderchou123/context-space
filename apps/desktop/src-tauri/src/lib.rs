mod bridge;
mod commands;
mod db;
mod events;
mod models;
mod tray;
mod windows;

use bridge::BridgeState;
use commands::CoreState;
use db::Database;
use events::EventLog;
use std::sync::Arc;
use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be registered first: a second launch focuses the running app instead of
        // creating a second tray icon and fighting over the bridge port.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = match Database::open(&data_dir.join("context-space.sqlite")) {
                Ok(db) => Arc::new(db),
                Err(e) => {
                    log::error!("database unavailable: {e}");
                    show_fatal_error(&e);
                    return Err(std::io::Error::other(e).into());
                }
            };
            let events = Arc::new(EventLog::new());
            if db.mark_active_as_recovered().unwrap_or(false) {
                events.record(
                    "WORKSPACE_RECOVERED",
                    "active workspace restored from previous run; exit actions paused",
                );
            }
            let bridge = Arc::new(BridgeState::new(events.clone()));
            bridge::start(db.clone(), bridge.clone());
            app.manage(CoreState::new(db, bridge, events));
            tray::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::save_workspace,
            commands::delete_workspace,
            commands::reorder_workspace,
            commands::add_browser_resource,
            commands::remove_browser_resource,
            commands::toggle_browser_resource,
            commands::add_app_resource,
            commands::remove_app_resource,
            commands::switch_workspace,
            commands::close_workspace,
            commands::restore_session,
            commands::get_debug_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running Context Space");
}

/// Startup failures happen before any window exists, so use a native message box.
#[cfg(target_os = "windows")]
fn show_fatal_error(message: &str) {
    use ::windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::HSTRING,
    };
    let text = HSTRING::from(format!(
        "{message}\n\nContext Space did not modify or delete this file. Move it aside or restore a backup, then start the app again."
    ));
    unsafe {
        MessageBoxW(
            None,
            &text,
            &HSTRING::from("Context Space"),
            MB_OK | MB_ICONERROR,
        );
    }
}
#[cfg(not(target_os = "windows"))]
fn show_fatal_error(_message: &str) {}
