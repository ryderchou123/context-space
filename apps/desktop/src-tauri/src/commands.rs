use crate::{
    bridge::{BRIDGE_PORT, BridgeState, CAPTURE_TIMEOUT},
    db::{self, ACTIVE_RECOVERED, ACTIVE_WORKSPACE, Database},
    events::EventLog,
    models::*,
    tray, windows,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, MutexGuard, TryLockError},
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};

pub const BUSY: &str = "Another workspace operation is still running. Try again in a moment.";
pub const EXTENSION_OFFLINE: &str = "Browser extension is not connected.";
const TRANSITIONS: &[&str] = &["WORKSPACE_SWITCH_COMPLETE", "WORKSPACE_CLOSE"];

pub struct CoreState {
    pub db: Arc<Database>,
    pub bridge: Arc<BridgeState>,
    pub events: Arc<EventLog>,
    pub switch_lock: Mutex<()>,
    pub capture_timeout: Duration,
}

impl CoreState {
    pub fn new(db: Arc<Database>, bridge: Arc<BridgeState>, events: Arc<EventLog>) -> Self {
        Self {
            db,
            bridge,
            events,
            switch_lock: Mutex::new(()),
            capture_timeout: CAPTURE_TIMEOUT,
        }
    }

    /// Rejects overlapping operations instead of queueing them, so double clicks and rapid
    /// tray clicks can never run two switches back to back or interleave with a delete.
    fn lock(&self) -> Result<MutexGuard<'_, ()>, String> {
        match self.switch_lock.try_lock() {
            Ok(guard) => Ok(guard),
            Err(TryLockError::WouldBlock) => Err(BUSY.into()),
            Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
        }
    }
}

fn snapshot(state: &CoreState) -> Result<AppSnapshot, String> {
    Ok(AppSnapshot {
        workspaces: state.db.list_workspaces()?,
        active_workspace_id: state.db.state(ACTIVE_WORKSPACE)?,
        active_workspace_recovered: state.db.state(ACTIVE_RECOVERED)?.is_some(),
        extension_connected: state.bridge.connected(),
        bridge_token: state.db.state("bridge_token")?.unwrap_or_default(),
        bridge_port: BRIDGE_PORT,
    })
}

/// Keeps the tray menu and every open dashboard in sync after a change.
pub fn notify_changed(app: &AppHandle) {
    tray::refresh(app);
    let _ = app.emit("workspace-changed", ());
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
    notify_changed(&app);
    snapshot(&state)
}
#[tauri::command]
pub fn delete_workspace(
    app: AppHandle,
    state: State<CoreState>,
    id: String,
) -> Result<AppSnapshot, String> {
    {
        let _guard = state.lock()?;
        state.db.delete_workspace(&id)?;
        state
            .events
            .record("WORKSPACE_DELETE", format!("workspace_id={id}"));
    }
    notify_changed(&app);
    snapshot(&state)
}
#[tauri::command]
pub fn reorder_workspace(
    app: AppHandle,
    state: State<CoreState>,
    id: String,
    direction: i64,
) -> Result<AppSnapshot, String> {
    state.db.reorder(&id, direction)?;
    notify_changed(&app);
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

// Switching waits for the extension and touches other processes, so these run off the
// main thread; running them synchronously froze the window and tray for seconds.
#[tauri::command(async)]
pub fn switch_workspace(
    app: AppHandle,
    state: State<'_, CoreState>,
    id: String,
    restore_session: Option<bool>,
) -> Result<OperationResult, String> {
    let result = switch_impl(&state, &id, restore_session);
    notify_changed(&app);
    result
}

pub fn switch_impl(
    state: &CoreState,
    id: &str,
    restore_override: Option<bool>,
) -> Result<OperationResult, String> {
    let _guard = state.lock()?;
    let target = state.db.workspace(id)?;
    let current_id = state.db.state(ACTIVE_WORKSPACE)?;
    let recovered = state.db.state(ACTIVE_RECOVERED)?.is_some();
    state.events.record(
        "WORKSPACE_SWITCH_START",
        format!("from={} to={id}", current_id.as_deref().unwrap_or("none")),
    );
    let mut warnings = vec![];
    let restore = restore_override.unwrap_or(target.restore_session_on_launch);
    let target_urls = workspace_urls(&target, restore);
    // A previous workspace that was deleted in the meantime must never block switching.
    let current = current_id
        .filter(|current| current != id)
        .and_then(|current| state.db.workspace(&current).ok());
    if let Some(current) = current {
        if recovered {
            warnings.push(format!(
                "Context Space restarted after {} was opened, so its tabs and apps were left as they are.",
                current.name
            ));
            state.events.record(
                "WORKSPACE_CLOSE",
                format!(
                    "workspace_id={} skipped=recovered_after_restart",
                    current.id
                ),
            );
        } else {
            leave_workspace(
                state,
                &current,
                &target_urls,
                &target.app_resources,
                &mut warnings,
            )?;
        }
    }
    open_urls(state, &target.id, target_urls, &mut warnings);
    warnings.extend(windows::launch_apps(&target.app_resources, &state.events));
    state.db.set_state(ACTIVE_WORKSPACE, Some(id))?;
    state.db.set_state(ACTIVE_RECOVERED, None)?;
    state.events.record(
        "WORKSPACE_SWITCH_COMPLETE",
        format!("to={id} warnings={}", warnings.len()),
    );
    Ok(OperationResult {
        message: format!("{} is now active.", target.name),
        warnings,
    })
}

/// Saves, then closes/minimizes what belongs to `workspace`, except resources the next
/// workspace also needs (`keep_urls`, `keep_apps`), which stay open instead of flickering.
fn leave_workspace(
    state: &CoreState,
    workspace: &Workspace,
    keep_urls: &[String],
    keep_apps: &[AppResource],
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let name = &workspace.name;
    if workspace.save_session_on_exit {
        if !state.bridge.connected() {
            warnings.push(format!(
                "{EXTENSION_OFFLINE} The current tabs of {name} could not be saved."
            ));
        } else {
            match state
                .bridge
                .wait_for_capture(&workspace.id, state.capture_timeout)
            {
                Some(tabs) => {
                    if state.db.save_captured_session(&workspace.id, &tabs)? {
                        state.events.record(
                            "SESSION_SAVE",
                            format!("workspace_id={} tabs={}", workspace.id, tabs.len()),
                        );
                    } else {
                        warnings.push(format!(
                            "No open tabs were found for {name}; its previous Last Session was kept."
                        ));
                    }
                }
                None => warnings.push(format!(
                    "The browser extension did not answer in time; the previous Last Session of {name} was kept."
                )),
            }
        }
    }
    // Re-read so tabs captured just now are part of what gets closed.
    let fresh = state
        .db
        .workspace(&workspace.id)
        .unwrap_or_else(|_| workspace.clone());
    if fresh.browser_exit_behavior == "close" {
        if state.bridge.connected() {
            let keep: HashSet<String> = keep_urls.iter().map(|u| db::url_key(u)).collect();
            let urls: Vec<String> = workspace_urls(&fresh, true)
                .into_iter()
                .filter(|u| !keep.contains(&db::url_key(u)))
                .collect();
            if !urls.is_empty() {
                state.events.record(
                    "TAB_CLOSE",
                    format!("workspace_id={} candidates={}", fresh.id, urls.len()),
                );
                state.bridge.push(BridgeCommand::CloseUrls {
                    workspace_id: fresh.id.clone(),
                    urls,
                });
            }
        } else {
            warnings.push(format!(
                "{EXTENSION_OFFLINE} No browser tabs of {name} were closed."
            ));
        }
    }
    warnings.extend(windows::apply_exit(
        &fresh.app_resources,
        &fresh.app_exit_behavior,
        keep_apps,
        &state.events,
    ));
    state
        .events
        .record("WORKSPACE_CLOSE", format!("workspace_id={}", fresh.id));
    Ok(())
}

#[tauri::command(async)]
pub fn close_workspace(
    app: AppHandle,
    state: State<'_, CoreState>,
    id: String,
) -> Result<OperationResult, String> {
    let result = close_impl(&state, &id);
    notify_changed(&app);
    result
}

pub fn close_impl(state: &CoreState, id: &str) -> Result<OperationResult, String> {
    let _guard = state.lock()?;
    let workspace = state.db.workspace(id)?;
    let mut warnings = vec![];
    // An explicit "Close workspace" is the user's decision, even after a restart.
    leave_workspace(state, &workspace, &[], &[], &mut warnings)?;
    if state.db.state(ACTIVE_WORKSPACE)?.as_deref() == Some(id) {
        state.db.set_state(ACTIVE_WORKSPACE, None)?;
        state.db.set_state(ACTIVE_RECOVERED, None)?;
    }
    Ok(OperationResult {
        message: format!("{} was closed safely.", workspace.name),
        warnings,
    })
}

#[tauri::command(async)]
pub fn restore_session(state: State<'_, CoreState>, id: String) -> Result<OperationResult, String> {
    restore_impl(&state, &id)
}

pub fn restore_impl(state: &CoreState, id: &str) -> Result<OperationResult, String> {
    let _guard = state.lock()?;
    let workspace = state.db.workspace(id)?;
    let mut warnings = vec![];
    let urls: Vec<String> = workspace
        .last_session
        .iter()
        .map(|t| t.url.clone())
        .collect();
    state.events.record(
        "SESSION_RESTORE",
        format!("workspace_id={id} tabs={}", urls.len()),
    );
    open_urls(state, &workspace.id, urls, &mut warnings);
    Ok(OperationResult {
        message: format!("Restored {} saved tabs.", workspace.last_session.len()),
        warnings,
    })
}

#[tauri::command(async)]
pub fn get_debug_info(state: State<'_, CoreState>) -> Result<DebugInfo, String> {
    debug_info(&state)
}

pub fn debug_info(state: &CoreState) -> Result<DebugInfo, String> {
    let active_id = state.db.state(ACTIVE_WORKSPACE)?;
    let active = active_id
        .as_deref()
        .and_then(|id| state.db.workspace(id).ok());
    let processes = windows::running_processes();
    Ok(DebugInfo {
        active_workspace_name: active.as_ref().map(|w| w.name.clone()),
        active_workspace_recovered: state.db.state(ACTIVE_RECOVERED)?.is_some(),
        extension_connected: state.bridge.connected(),
        extension_client_count: state.bridge.client_count(),
        extension_last_seen_secs: state.bridge.last_seen_secs(),
        database_path: state.db.path().into(),
        database_status: match state.db.list_workspaces() {
            Ok(all) => format!("ok ({} workspaces)", all.len()),
            Err(e) => format!("error: {e}"),
        },
        schema_version: state.db.schema_version()?,
        last_session_tabs: active.as_ref().map_or(0, |w| w.last_session.len()),
        tracked_apps: active
            .as_ref()
            .map(|w| {
                w.app_resources
                    .iter()
                    .map(|a| TrackedApp {
                        display_name: a.display_name.clone(),
                        running: !windows::pids_for(a, &processes).is_empty(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        active_workspace_id: active_id,
        last_transition: state.events.last_of(TRANSITIONS),
        recent_events: state.events.recent().into_iter().take(50).collect(),
    })
}

fn open_urls(state: &CoreState, workspace_id: &str, urls: Vec<String>, warnings: &mut Vec<String>) {
    if urls.is_empty() {
        return;
    }
    state.events.record(
        "TAB_OPEN",
        format!("workspace_id={workspace_id} count={}", urls.len()),
    );
    if state.bridge.connected() {
        state.bridge.push(BridgeCommand::OpenUrls {
            workspace_id: workspace_id.to_string(),
            urls,
        });
        return;
    }
    let mut opened = 0;
    for url in urls {
        match open_in_default_browser(&url) {
            Ok(()) => opened += 1,
            Err(e) => warnings.push(format!("Could not open {url}: {e}")),
        }
    }
    if opened > 0 {
        warnings.push(format!(
            "{EXTENSION_OFFLINE} {opened} page(s) opened in your default browser, so tabs that were already open could not be reused."
        ));
    }
}

#[cfg(not(test))]
fn open_in_default_browser(url: &str) -> std::io::Result<()> {
    open::that(url)
}
#[cfg(test)]
fn open_in_default_browser(_url: &str) -> std::io::Result<()> {
    Ok(())
}

/// Enabled pinned resources plus, optionally, the Last Session; duplicates removed.
pub fn workspace_urls(w: &Workspace, include_session: bool) -> Vec<String> {
    let mut seen = HashSet::new();
    let session = w.last_session.iter().filter(|_| include_session);
    w.browser_resources
        .iter()
        .filter(|r| r.enabled && r.pinned)
        .map(|r| r.url.clone())
        .chain(session.map(|t| t.url.clone()))
        .filter(|url| seen.insert(db::url_key(url)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::tests::request;
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::Instant,
    };

    fn draft(name: &str, browser: &str, apps: &str) -> WorkspaceDraft {
        WorkspaceDraft {
            id: None,
            name: name.into(),
            icon: "briefcase".into(),
            color: "#0D9488".into(),
            browser_exit_behavior: browser.into(),
            app_exit_behavior: apps.into(),
            save_session_on_exit: true,
            restore_session_on_launch: true,
        }
    }
    fn state() -> CoreState {
        let events = Arc::new(EventLog::new());
        let mut state = CoreState::new(
            Arc::new(Database::memory()),
            Arc::new(BridgeState::new(events.clone())),
            events,
        );
        state.capture_timeout = Duration::from_millis(300);
        state
    }
    fn active(state: &CoreState) -> Option<String> {
        state.db.state(ACTIVE_WORKSPACE).unwrap()
    }

    /// Workspace id -> (title, url) of tabs the fake extension owns.
    type OwnedTabs = HashMap<String, Vec<(String, String)>>;

    /// Plays the browser extension over the real HTTP bridge: records every command and
    /// answers captures with the tabs it "owns" per workspace.
    struct FakeExtension {
        commands: Arc<Mutex<Vec<serde_json::Value>>>,
        owned: Arc<Mutex<OwnedTabs>>,
        stop: Arc<AtomicBool>,
    }
    impl FakeExtension {
        fn start(state: &CoreState) -> Self {
            let port = crate::bridge::start_on(0, state.db.clone(), state.bridge.clone()).unwrap();
            let token = state.db.state("bridge_token").unwrap().unwrap();
            let fake = Self {
                commands: Arc::default(),
                owned: Arc::default(),
                stop: Arc::default(),
            };
            let (commands, owned, stop) =
                (fake.commands.clone(), fake.owned.clone(), fake.stop.clone());
            // One status call marks the extension as connected before the first switch.
            request(port, "GET", "/api/status", &token, "");
            thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let (_, body) = request(port, "GET", "/api/poll", &token, "");
                    let Ok(command) = serde_json::from_str::<serde_json::Value>(&body) else {
                        continue;
                    };
                    if command.is_null() {
                        continue;
                    }
                    if command["type"] == "capture_tabs" {
                        let ws = command["workspace_id"].as_str().unwrap().to_string();
                        let tabs: Vec<_> = owned
                            .lock()
                            .unwrap()
                            .get(&ws)
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(title, url)| serde_json::json!({ "title": title, "url": url }))
                            .collect();
                        let payload = serde_json::json!({ "requestId": command["request_id"], "workspaceId": ws, "tabs": tabs });
                        request(port, "POST", "/api/capture", &token, &payload.to_string());
                    }
                    commands.lock().unwrap().push(command);
                }
            });
            fake
        }
        fn own(&self, workspace: &str, urls: &[&str]) {
            self.owned.lock().unwrap().insert(
                workspace.into(),
                urls.iter()
                    .map(|u| (u.to_string(), u.to_string()))
                    .collect(),
            );
        }
        /// Waits until at least `n` commands arrived, then returns them.
        fn wait_for(&self, n: usize) -> Vec<serde_json::Value> {
            let started = Instant::now();
            while self.commands.lock().unwrap().len() < n
                && started.elapsed() < Duration::from_secs(3)
            {
                thread::sleep(Duration::from_millis(10));
            }
            self.commands.lock().unwrap().clone()
        }
        fn clear(&self) {
            self.commands.lock().unwrap().clear();
        }
    }
    impl Drop for FakeExtension {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
        }
    }
    fn urls(command: &serde_json::Value) -> Vec<String> {
        command["urls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u.as_str().unwrap().to_string())
            .collect()
    }
    fn of_type<'a>(commands: &'a [serde_json::Value], kind: &str) -> Vec<&'a serde_json::Value> {
        commands.iter().filter(|c| c["type"] == kind).collect()
    }

    #[test]
    fn switch_plan_merges_pinned_and_session_urls() {
        let db = Database::memory();
        let id = db.save_workspace(draft("Work", "close", "keep")).unwrap();
        db.add_browser(&id, "Example", "https://example.com/", true)
            .unwrap();
        db.add_browser(&id, "Saved", "https://saved.example", false)
            .unwrap();
        db.save_session(
            &id,
            &[
                SessionTab {
                    title: "Dup".into(),
                    url: "https://example.com".into(),
                },
                SessionTab {
                    title: "Docs".into(),
                    url: "https://example.com/docs".into(),
                },
            ],
        )
        .unwrap();
        let workspace = db.workspace(&id).unwrap();
        assert_eq!(
            workspace_urls(&workspace, true),
            vec!["https://example.com", "https://example.com/docs"]
        );
        assert_eq!(
            workspace_urls(&workspace, false),
            vec!["https://example.com"]
        );
    }

    #[test]
    fn switching_handles_no_active_same_deleted_and_other_workspace() {
        let state = state();
        let work = state
            .db
            .save_workspace(draft("Work", "keep", "keep"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "keep", "keep"))
            .unwrap();
        switch_impl(&state, &work, None).unwrap();
        assert_eq!(active(&state), Some(work.clone()));
        switch_impl(&state, &work, None).unwrap();
        switch_impl(&state, &games, None).unwrap();
        assert_eq!(active(&state), Some(games.clone()));
        state.db.delete_workspace(&work).unwrap();
        assert_eq!(
            switch_impl(&state, &work, None).unwrap_err(),
            db::MISSING_WORKSPACE
        );
        assert_eq!(active(&state), Some(games));
    }

    #[test]
    fn flow_3_work_to_games_saves_closes_only_work_tabs_and_activates_games() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let work = state
            .db
            .save_workspace(draft("Work", "close", "minimize"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "close", "minimize"))
            .unwrap();
        state
            .db
            .add_browser(&work, "LinkedIn", "https://linkedin.com", true)
            .unwrap();
        state
            .db
            .add_browser(&games, "Twitch", "https://twitch.tv", true)
            .unwrap();

        switch_impl(&state, &work, None).unwrap();
        let opened = fake.wait_for(1);
        assert_eq!(
            urls(of_type(&opened, "open_urls")[0]),
            vec!["https://linkedin.com"]
        );
        fake.clear();

        fake.own(
            &work,
            &["https://linkedin.com/", "https://linkedin.com/jobs"],
        );
        let result = switch_impl(&state, &games, None).unwrap();
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        let commands = fake.wait_for(3);
        assert_eq!(
            commands
                .iter()
                .map(|c| c["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["capture_tabs", "close_urls", "open_urls"]
        );
        let close = of_type(&commands, "close_urls")[0];
        assert_eq!(close["workspace_id"], work.as_str());
        assert_eq!(
            urls(close),
            vec!["https://linkedin.com", "https://linkedin.com/jobs"]
        );
        assert_eq!(
            urls(of_type(&commands, "open_urls")[0]),
            vec!["https://twitch.tv"]
        );
        let saved = state.db.workspace(&work).unwrap().last_session;
        assert_eq!(saved.len(), 2, "Work session is saved");
        assert_eq!(active(&state), Some(games));
    }

    #[test]
    fn flow_4_returning_to_work_restores_the_captured_session() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let work = state
            .db
            .save_workspace(draft("Work", "close", "keep"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "close", "keep"))
            .unwrap();
        state
            .db
            .add_browser(&work, "LinkedIn", "https://linkedin.com", true)
            .unwrap();
        switch_impl(&state, &work, None).unwrap();
        fake.own(
            &work,
            &[
                "https://linkedin.com",
                "https://docs.rs/tauri",
                "https://github.com/pulls",
            ],
        );
        switch_impl(&state, &games, None).unwrap();
        fake.wait_for(3);
        fake.clear();

        switch_impl(&state, &work, None).unwrap();
        let commands = fake.wait_for(2);
        let open = of_type(&commands, "open_urls");
        assert_eq!(
            urls(open[0]),
            vec![
                "https://linkedin.com",
                "https://docs.rs/tauri",
                "https://github.com/pulls"
            ]
        );
        fake.clear();
        restore_impl(&state, &work).unwrap();
        let restored = fake.wait_for(1);
        assert_eq!(urls(of_type(&restored, "open_urls")[0]).len(), 3);
    }

    #[test]
    fn regression_bug_006_urls_shared_with_the_next_workspace_stay_open() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let work = state
            .db
            .save_workspace(draft("Work", "close", "keep"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "close", "keep"))
            .unwrap();
        state
            .db
            .add_browser(&work, "LinkedIn", "https://linkedin.com", true)
            .unwrap();
        state
            .db
            .add_browser(&work, "Mail", "https://mail.example", true)
            .unwrap();
        state
            .db
            .add_browser(&games, "LinkedIn", "https://linkedin.com/", true)
            .unwrap();
        switch_impl(&state, &work, None).unwrap();
        fake.own(&work, &["https://linkedin.com", "https://mail.example"]);
        switch_impl(&state, &games, None).unwrap();
        let commands = fake.wait_for(4);
        assert_eq!(
            urls(of_type(&commands, "close_urls")[0]),
            vec!["https://mail.example"]
        );
    }

    #[test]
    fn keep_tabs_setting_never_sends_a_close_command() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let work = state
            .db
            .save_workspace(draft("Work", "keep", "keep"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "keep", "keep"))
            .unwrap();
        state
            .db
            .add_browser(&work, "LinkedIn", "https://linkedin.com", true)
            .unwrap();
        switch_impl(&state, &work, None).unwrap();
        fake.own(&work, &["https://linkedin.com"]);
        switch_impl(&state, &games, None).unwrap();
        let commands = fake.wait_for(2);
        assert!(of_type(&commands, "close_urls").is_empty());
    }

    #[test]
    fn regression_bug_009_switch_with_no_owned_tabs_keeps_last_session() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let work = state
            .db
            .save_workspace(draft("Work", "close", "keep"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "close", "keep"))
            .unwrap();
        state
            .db
            .save_session(
                &work,
                &[SessionTab {
                    title: "A".into(),
                    url: "https://a.test".into(),
                }],
            )
            .unwrap();
        state.db.set_state(ACTIVE_WORKSPACE, Some(&work)).unwrap();
        let result = switch_impl(&state, &games, None).unwrap();
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("previous Last Session was kept"))
        );
        assert_eq!(state.db.workspace(&work).unwrap().last_session.len(), 1);
        drop(fake);
    }

    #[test]
    fn extension_offline_degrades_with_clear_warnings_and_no_commands() {
        let state = state();
        let work = state
            .db
            .save_workspace(draft("Work", "close", "minimize"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "close", "minimize"))
            .unwrap();
        state
            .db
            .add_browser(&games, "Twitch", "https://twitch.tv", true)
            .unwrap();
        switch_impl(&state, &work, None).unwrap();
        let result = switch_impl(&state, &games, None).unwrap();
        let text = result.warnings.join("\n");
        assert!(text.contains(EXTENSION_OFFLINE), "{text}");
        assert!(text.contains("could not be saved"), "{text}");
        assert!(
            text.contains("No browser tabs of Work were closed"),
            "{text}"
        );
        assert!(text.contains("default browser"), "{text}");
        assert!(state.bridge_queue_is_empty());
        assert_eq!(active(&state), Some(games));
    }

    #[test]
    fn capture_timeout_keeps_previous_session_and_reports_it() {
        let state = state();
        let work = state
            .db
            .save_workspace(draft("Work", "keep", "keep"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "keep", "keep"))
            .unwrap();
        state
            .db
            .save_session(
                &work,
                &[SessionTab {
                    title: "A".into(),
                    url: "https://a.test".into(),
                }],
            )
            .unwrap();
        state.db.set_state(ACTIVE_WORKSPACE, Some(&work)).unwrap();
        state.bridge.mark_seen_for_tests(); // "connected", but nobody answers
        let result = switch_impl(&state, &games, None).unwrap();
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("did not answer in time"))
        );
        assert_eq!(state.db.workspace(&work).unwrap().last_session.len(), 1);
    }

    #[test]
    fn regression_bug_010_recovered_state_does_not_close_anything_after_restart() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let work = state
            .db
            .save_workspace(draft("Work", "close", "safe_close"))
            .unwrap();
        let games = state
            .db
            .save_workspace(draft("Games", "close", "keep"))
            .unwrap();
        state
            .db
            .add_browser(&work, "LinkedIn", "https://linkedin.com", true)
            .unwrap();
        state.db.set_state(ACTIVE_WORKSPACE, Some(&work)).unwrap();
        assert!(state.db.mark_active_as_recovered().unwrap()); // simulated restart
        fake.own(&work, &["https://linkedin.com"]);
        let result = switch_impl(&state, &games, None).unwrap();
        assert!(result.warnings.iter().any(|w| w.contains("restarted")));
        thread::sleep(Duration::from_millis(200));
        let commands = fake.wait_for(0);
        assert!(of_type(&commands, "capture_tabs").is_empty());
        assert!(of_type(&commands, "close_urls").is_empty());
        assert_eq!(state.db.state(ACTIVE_RECOVERED).unwrap(), None);
        assert_eq!(active(&state), Some(games));
    }

    #[test]
    fn regression_bug_017_overlapping_operations_are_rejected_not_queued() {
        let state = state();
        let work = state
            .db
            .save_workspace(draft("Work", "keep", "keep"))
            .unwrap();
        let _held = state.lock().unwrap();
        assert_eq!(switch_impl(&state, &work, None).unwrap_err(), BUSY);
        assert_eq!(close_impl(&state, &work).unwrap_err(), BUSY);
        assert_eq!(restore_impl(&state, &work).unwrap_err(), BUSY);
        assert_eq!(active(&state), None);
    }

    #[test]
    fn regression_bug_017_active_pointing_to_a_missing_workspace_does_not_block_switching() {
        let state = state();
        let games = state
            .db
            .save_workspace(draft("Games", "keep", "keep"))
            .unwrap();
        state
            .db
            .set_state(ACTIVE_WORKSPACE, Some("deleted-elsewhere"))
            .unwrap();
        switch_impl(&state, &games, None).unwrap();
        assert_eq!(active(&state), Some(games));
    }

    #[test]
    fn rapid_switching_cycle_ends_consistent_without_duplicate_urls() {
        let state = state();
        let fake = FakeExtension::start(&state);
        let ids: Vec<_> = ["Work", "Games", "Homework"]
            .iter()
            .map(|n| state.db.save_workspace(draft(n, "close", "keep")).unwrap())
            .collect();
        for id in &ids {
            state
                .db
                .add_browser(id, "Shared", "https://shared.example", true)
                .unwrap();
            state
                .db
                .add_browser(id, "Own", &format!("https://{id}.example"), true)
                .unwrap();
        }
        for id in [&ids[0], &ids[1], &ids[2], &ids[0]] {
            switch_impl(&state, id, None).unwrap();
        }
        assert_eq!(active(&state), Some(ids[0].clone()));
        for command in of_type(&fake.wait_for(4), "open_urls") {
            let list = urls(command);
            let unique: HashSet<_> = list.iter().collect();
            assert_eq!(unique.len(), list.len(), "{list:?}");
        }
    }

    #[test]
    fn concurrent_switches_from_many_threads_leave_a_valid_active_workspace() {
        let state = Arc::new(state());
        let ids: Vec<_> = ["Work", "Games", "Homework"]
            .iter()
            .map(|n| state.db.save_workspace(draft(n, "keep", "keep")).unwrap())
            .collect();
        let handles: Vec<_> = (0..12)
            .map(|i| {
                let (state, id) = (state.clone(), ids[i % 3].clone());
                thread::spawn(move || switch_impl(&state, &id, None))
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(results.iter().any(Result::is_ok));
        assert!(
            results
                .iter()
                .all(|r| r.as_ref().map_or_else(|e| e == BUSY, |_| true))
        );
        assert!(ids.contains(&active(&state).unwrap()));
    }

    #[test]
    fn close_workspace_clears_active_state_and_logs_the_transition() {
        let state = state();
        let work = state
            .db
            .save_workspace(draft("Work", "keep", "keep"))
            .unwrap();
        switch_impl(&state, &work, None).unwrap();
        close_impl(&state, &work).unwrap();
        assert_eq!(active(&state), None);
        let info = debug_info(&state).unwrap();
        assert_eq!(info.last_transition.unwrap().event, "WORKSPACE_CLOSE");
        assert_eq!(info.schema_version, db::SCHEMA_VERSION);
        let names = state.events.names();
        for expected in [
            "WORKSPACE_SWITCH_START",
            "WORKSPACE_SWITCH_COMPLETE",
            "WORKSPACE_CLOSE",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "{expected} missing from {names:?}"
            );
        }
    }

    impl CoreState {
        fn bridge_queue_is_empty(&self) -> bool {
            self.bridge.queue_len_for_tests() == 0
        }
    }
}
