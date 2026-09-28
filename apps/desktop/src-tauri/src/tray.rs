use crate::commands::{CoreState, switch_impl};
use tauri::{
    App, AppHandle, Manager,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

pub fn setup(app: &App) -> tauri::Result<()> {
    let menu = build_menu(app.handle())?;
    let mut builder = TrayIconBuilder::with_id("context-space-tray")
        .tooltip("Context Space")
        .menu(&menu)
        .on_menu_event(|app, event| handle_event(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone())
    }
    builder.build(app)?;
    Ok(())
}
pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("context-space-tray") {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}
fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let state = app.state::<CoreState>();
    let workspaces = state.db.list_workspaces().unwrap_or_default();
    let active = state.db.state("active_workspace_id").ok().flatten();
    let active_name = active
        .as_ref()
        .and_then(|id| workspaces.iter().find(|w| &w.id == id))
        .map(|w| w.name.as_str())
        .unwrap_or("None");
    let active_item = MenuItemBuilder::with_id("active", format!("Active: {active_name}"))
        .enabled(false)
        .build(app)?;
    let mut menu = MenuBuilder::new(app)
        .item(&active_item)
        .item(&PredefinedMenuItem::separator(app)?);
    for workspace in workspaces {
        let item = MenuItemBuilder::with_id(
            format!("switch:{}", workspace.id),
            format!("Switch to {}", workspace.name),
        )
        .build(app)?;
        menu = menu.item(&item);
    }
    let open = MenuItemBuilder::with_id("open", "Open Dashboard").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Context Space").build(app)?;
    menu.item(&PredefinedMenuItem::separator(app)?)
        .item(&open)
        .item(&quit)
        .build()
}
fn handle_event(app: &AppHandle, id: &str) {
    match id {
        "open" => show(app),
        "quit" => app.exit(0),
        _ if id.starts_with("switch:") => {
            let state = app.state::<CoreState>();
            if let Err(e) = switch_impl(&state, &id[7..], None) {
                log::error!("tray switch failed: {e}")
            }
            refresh(app);
        }
        _ => {}
    }
}
pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
