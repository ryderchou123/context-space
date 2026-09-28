mod bridge;
mod commands;
mod db;
mod models;
mod tray;
mod windows;

use bridge::BridgeState;
use commands::CoreState;
use db::Database;
use std::sync::Arc;
use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            let db = Arc::new(
                Database::open(&data_dir.join("context-space.sqlite"))
                    .map_err(std::io::Error::other)?,
            );
            let bridge = Arc::new(BridgeState::new());
            bridge::start(db.clone(), bridge.clone());
            app.manage(CoreState { db, bridge });
            tray::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
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
            commands::restore_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running Context Space");
}
