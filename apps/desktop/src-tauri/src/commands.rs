use crate::{
    bridge::{BRIDGE_PORT, BridgeState},
    db::Database,
    models::*,
    tray, windows,
};
use std::{collections::HashSet, sync::Arc};
use tauri::{AppHandle, State};

pub struct CoreState {
    pub db: Arc<Database>,
    pub bridge: Arc<BridgeState>,
}

fn snapshot(state: &CoreState) -> Result<AppSnapshot, String> {
    Ok(AppSnapshot {
        workspaces: state.db.list_workspaces()?,
        active_workspace_id: state.db.state("active_workspace_id")?,
        extension_connected: state.bridge.connected(),
        bridge_token: state.db.state("bridge_token")?.unwrap_or_default(),
        bridge_port: BRIDGE_PORT,
    })
}
#[tauri::command]
pub fn get_snapshot(state: State<CoreState>) -> Result<AppSnapshot, String> {
    snapshot(&state)
}
#[tauri::command]
pub fn save_workspace(
    app: AppHandle,
    state: State<CoreState>,
    draft: WorkspaceDraft,
) -> Result<AppSnapshot, String> {
    state.db.save_workspace(draft)?;
    tray::refresh(&app);
    snapshot(&state)
}
#[tauri::command]
pub fn delete_workspace(
    app: AppHandle,
    state: State<CoreState>,
    id: String,
) -> Result<AppSnapshot, String> {
    state.db.delete_workspace(&id)?;
    tray::refresh(&app);
    snapshot(&state)
}
#[tauri::command]
pub fn reorder_workspace(
    state: State<CoreState>,
    id: String,
    direction: i64,
) -> Result<AppSnapshot, String> {
    state.db.reorder(&id, direction)?;
    snapshot(&state)
}
#[tauri::command]
pub fn add_browser_resource(
    state: State<CoreState>,
    workspace_id: String,
    title: String,
    url: String,
    pinned: bool,
) -> Result<AppSnapshot, String> {
    state.db.add_browser(&workspace_id, &title, &url, pinned)?;
    snapshot(&state)
}
#[tauri::command]
pub fn remove_browser_resource(state: State<CoreState>, id: String) -> Result<AppSnapshot, String> {
    state.db.remove_browser(&id)?;
    snapshot(&state)
}
#[tauri::command]
pub fn toggle_browser_resource(
    state: State<CoreState>,
    id: String,
    enabled: bool,
    pinned: bool,
) -> Result<AppSnapshot, String> {
    state.db.toggle_browser(&id, enabled, pinned)?;
    snapshot(&state)
}
#[tauri::command]
pub fn add_app_resource(
    state: State<CoreState>,
    workspace_id: String,
    display_name: String,
    executable_path: String,
    process_name: String,
    launch_args: String,
) -> Result<AppSnapshot, String> {
    state.db.add_app(
        &workspace_id,
        &display_name,
        &executable_path,
        &process_name,
        &launch_args,
    )?;
    snapshot(&state)
}
#[tauri::command]
pub fn remove_app_resource(state: State<CoreState>, id: String) -> Result<AppSnapshot, String> {
    state.db.remove_app(&id)?;
    snapshot(&state)
}

#[tauri::command]
pub fn switch_workspace(
    app: AppHandle,
    state: State<CoreState>,
    id: String,
    restore_session: Option<bool>,
) -> Result<OperationResult, String> {
    let result = switch_impl(&state, &id, restore_session)?;
    tray::refresh(&app);
    Ok(result)
}
pub fn switch_impl(
    state: &CoreState,
    id: &str,
    restore_override: Option<bool>,
) -> Result<OperationResult, String> {
    let target = state.db.workspace(id)?;
    let mut warnings = vec![];
    if let Some(current_id) = state.db.state("active_workspace_id")? {
        if current_id != id {
            let current = state.db.workspace(&current_id)?;
            if current.save_session_on_exit {
                if state.bridge.connected() {
                    if let Some(tabs) = state.bridge.wait_for_capture(&current.id) {
                        state.db.save_session(&current.id, &dedupe_tabs(tabs))?
                    } else {
                        warnings.push("The browser extension did not answer in time; the previous saved session was kept.".into())
                    }
                } else {
                    warnings.push(
                        "The browser extension is offline, so current tabs could not be captured."
                            .into(),
                    )
                }
            }
            if current.browser_exit_behavior == "close" {
                state.bridge.push(BridgeCommand::CloseUrls {
                    urls: workspace_urls(&current, true),
                });
            }
            warnings.extend(windows::apply_exit(
                &current.app_resources,
                &current.app_exit_behavior,
            ));
        }
    }
    let restore = restore_override.unwrap_or(target.restore_session_on_launch);
    open_urls(state, workspace_urls(&target, restore), &mut warnings);
    warnings.extend(windows::launch_apps(&target.app_resources));
    state.db.set_state("active_workspace_id", Some(id))?;
    Ok(OperationResult {
        message: format!("{} is now active.", target.name),
        warnings,
    })
}

#[tauri::command]
pub fn close_workspace(
    app: AppHandle,
    state: State<CoreState>,
    id: String,
) -> Result<OperationResult, String> {
    let workspace = state.db.workspace(&id)?;
    let mut warnings = vec![];
    if workspace.save_session_on_exit && state.bridge.connected() {
        if let Some(tabs) = state.bridge.wait_for_capture(&id) {
            state.db.save_session(&id, &dedupe_tabs(tabs))?
        }
    }
    if workspace.browser_exit_behavior == "close" {
        state.bridge.push(BridgeCommand::CloseUrls {
            urls: workspace_urls(&workspace, true),
        });
    }
    warnings.extend(windows::apply_exit(
        &workspace.app_resources,
        &workspace.app_exit_behavior,
    ));
    if state.db.state("active_workspace_id")?.as_deref() == Some(&id) {
        state.db.set_state("active_workspace_id", None)?;
    }
    tray::refresh(&app);
    Ok(OperationResult {
        message: format!("{} was closed safely.", workspace.name),
        warnings,
    })
}
#[tauri::command]
pub fn restore_session(state: State<CoreState>, id: String) -> Result<OperationResult, String> {
    let workspace = state.db.workspace(&id)?;
    let mut warnings = vec![];
    open_urls(
        &state,
        workspace
            .last_session
            .iter()
            .map(|t| t.url.clone())
            .collect(),
        &mut warnings,
    );
    Ok(OperationResult {
        message: format!("Restored {} saved tabs.", workspace.last_session.len()),
        warnings,
    })
}

fn open_urls(state: &CoreState, urls: Vec<String>, warnings: &mut Vec<String>) {
    if urls.is_empty() {
        return;
    }
    if state.bridge.connected() {
        state.bridge.push(BridgeCommand::OpenUrls { urls })
    } else {
        for url in urls {
            if let Err(e) = open::that(&url) {
                warnings.push(format!("Could not open {url}: {e}"))
            }
        }
    }
}
fn workspace_urls(w: &Workspace, include_session: bool) -> Vec<String> {
    let mut seen = HashSet::new();
    w.browser_resources
        .iter()
        .filter(|r| r.enabled && r.pinned)
        .map(|r| r.url.clone())
        .chain(if include_session {
            w.last_session
                .iter()
                .map(|t| t.url.clone())
                .collect::<Vec<_>>()
        } else {
            vec![]
        })
        .filter(|url| seen.insert(normalize_key(url)))
        .collect()
}
fn normalize_key(url: &str) -> String {
    url.trim_end_matches('/').to_lowercase()
}
fn dedupe_tabs(tabs: Vec<SessionTab>) -> Vec<SessionTab> {
    let mut seen = HashSet::new();
    tabs.into_iter()
        .filter(|t| url::Url::parse(&t.url).is_ok() && seen.insert(normalize_key(&t.url)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tab_dedupe_normalizes_trailing_slash() {
        let tabs = vec![
            SessionTab {
                title: "A".into(),
                url: "https://example.com".into(),
            },
            SessionTab {
                title: "B".into(),
                url: "https://example.com/".into(),
            },
        ];
        assert_eq!(dedupe_tabs(tabs).len(), 1)
    }
    #[test]
    fn switch_plan_merges_pinned_and_session_urls() {
        let workspace = Workspace {
            id: "1".into(),
            name: "Work".into(),
            icon: "briefcase".into(),
            color: "#0D9488".into(),
            position: 0,
            browser_exit_behavior: "close".into(),
            app_exit_behavior: "minimize".into(),
            save_session_on_exit: true,
            restore_session_on_launch: true,
            browser_resources: vec![BrowserResource {
                id: "b".into(),
                workspace_id: "1".into(),
                title: "Example".into(),
                url: "https://example.com/".into(),
                pinned: true,
                enabled: true,
                position: 0,
            }],
            app_resources: vec![],
            last_session: vec![
                SessionTab {
                    title: "Duplicate".into(),
                    url: "https://example.com".into(),
                },
                SessionTab {
                    title: "Docs".into(),
                    url: "https://example.com/docs".into(),
                },
            ],
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert_eq!(
            workspace_urls(&workspace, true),
            vec!["https://example.com/", "https://example.com/docs"]
        );
        assert_eq!(
            workspace_urls(&workspace, false),
            vec!["https://example.com/"]
        );
    }
}
