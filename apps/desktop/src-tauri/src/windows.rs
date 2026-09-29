use crate::{events::EventLog, models::AppResource};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};
use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};

/// Closing a browser window closes every tab in it, including other workspaces' tabs.
const BROWSER_PROCESSES: &[&str] = &[
    "chrome", "msedge", "firefox", "brave", "opera", "vivaldi", "arc", "iexplore",
];

#[derive(Debug, Clone)]
pub struct RunningProcess {
    pub pid: u32,
    pub name: String,
    pub exe: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitAction {
    Minimize,
    SafeClose,
}

#[derive(Debug)]
pub struct ExitStep<'a> {
    pub app: &'a AppResource,
    pub pids: HashSet<u32>,
    pub action: ExitAction,
    pub note: Option<String>,
}

/// One process snapshot per operation; refreshing everything per app took seconds.
pub fn running_processes() -> Vec<RunningProcess> {
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)),
    );
    system
        .processes()
        .values()
        .map(|p| RunningProcess {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            exe: p.exe().map(Path::to_path_buf),
        })
        .collect()
}

fn process_key(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}
fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}
fn app_process_key(app: &AppResource) -> String {
    let explicit = process_key(&app.process_name);
    if !explicit.is_empty() {
        return explicit;
    }
    Path::new(&app.executable_path)
        .file_name()
        .map(|f| process_key(&f.to_string_lossy()))
        .unwrap_or_default()
}

/// Two resources refer to the same program when their executable or process name match.
pub fn same_app(a: &AppResource, b: &AppResource) -> bool {
    same_path(Path::new(&a.executable_path), Path::new(&b.executable_path))
        || (!app_process_key(a).is_empty() && app_process_key(a) == app_process_key(b))
}

pub fn is_browser(app: &AppResource) -> bool {
    BROWSER_PROCESSES.contains(&app_process_key(app).as_str())
}

pub fn pids_for(app: &AppResource, table: &[RunningProcess]) -> HashSet<u32> {
    let wanted_name = app_process_key(app);
    let wanted_path = Path::new(&app.executable_path);
    let own_pid = std::process::id();
    table
        .iter()
        .filter(|p| p.pid != own_pid)
        .filter(|p| {
            (!wanted_name.is_empty() && process_key(&p.name) == wanted_name)
                || p.exe.as_deref().is_some_and(|e| same_path(e, wanted_path))
        })
        .map(|p| p.pid)
        .collect()
}

/// Splits launch arguments the Windows way: whitespace separates, double quotes group,
/// and backslashes are literal so `C:\Users\me` survives (POSIX shlex dropped them).
pub fn split_args(input: &str) -> Vec<String> {
    let mut args = vec![];
    let mut current = String::new();
    let (mut quoted, mut pending) = (false, false);
    for c in input.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                pending = true;
            }
            c if c.is_whitespace() && !quoted => {
                if pending {
                    args.push(std::mem::take(&mut current));
                    pending = false;
                }
            }
            c => {
                current.push(c);
                pending = true;
            }
        }
    }
    if pending {
        args.push(current);
    }
    args
}

pub fn plan_launch<'a>(
    apps: &'a [AppResource],
    table: &[RunningProcess],
    exists: impl Fn(&Path) -> bool,
) -> (Vec<&'a AppResource>, Vec<String>) {
    let mut launch: Vec<&AppResource> = vec![];
    let mut warnings = vec![];
    for app in apps.iter().filter(|a| a.enabled) {
        if !exists(Path::new(&app.executable_path)) {
            warnings.push(format!(
                "{} was skipped because its executable is missing or was moved.",
                app.display_name
            ));
        } else if pids_for(app, table).is_empty() && !launch.iter().any(|l| same_app(l, app)) {
            launch.push(app);
        }
    }
    (launch, warnings)
}

pub fn launch_apps(apps: &[AppResource], events: &EventLog) -> Vec<String> {
    let (launch, mut warnings) = plan_launch(apps, &running_processes(), Path::is_file);
    for app in launch {
        let path = Path::new(&app.executable_path);
        let mut command = Command::new(path);
        command.args(split_args(&app.launch_args));
        if let Some(dir) = path.parent() {
            command.current_dir(dir);
        }
        match command.spawn() {
            Ok(_) => events.record("APP_LAUNCH", format!("app={}", app.display_name)),
            Err(e) => warnings.push(format!("Could not open {}: {e}", app.display_name)),
        }
    }
    warnings
}

pub fn plan_exit<'a>(
    apps: &'a [AppResource],
    default_behavior: &str,
    keep: &[AppResource],
    table: &[RunningProcess],
) -> Vec<ExitStep<'a>> {
    apps.iter()
        .filter(|a| a.enabled)
        // Apps the next workspace also uses stay untouched instead of closing and relaunching.
        .filter(|a| !keep.iter().any(|k| k.enabled && same_app(a, k)))
        .filter_map(|app| {
            let behavior = app
                .exit_behavior_override
                .as_deref()
                .unwrap_or(default_behavior);
            let (action, note) = match behavior {
                "minimize" => (ExitAction::Minimize, None),
                "safe_close" if is_browser(app) => (
                    ExitAction::Minimize,
                    Some(format!(
                        "{} is a web browser, so it was minimized instead of closed to protect tabs from other workspaces.",
                        app.display_name
                    )),
                ),
                "safe_close" => (ExitAction::SafeClose, None),
                _ => return None,
            };
            let pids = pids_for(app, table);
            (!pids.is_empty()).then_some(ExitStep {
                app,
                pids,
                action,
                note,
            })
        })
        .collect()
}

/// Minimizes or politely asks apps to close (WM_CLOSE, so unsaved-work prompts still
/// appear). Never terminates a process.
pub fn apply_exit(
    apps: &[AppResource],
    default_behavior: &str,
    keep: &[AppResource],
    events: &EventLog,
) -> Vec<String> {
    let table = running_processes();
    let mut warnings = vec![];
    for step in plan_exit(apps, default_behavior, keep, &table) {
        warnings.extend(step.note.clone());
        let name = &step.app.display_name;
        let closed = step.action == ExitAction::SafeClose;
        match window_action(&step.pids, closed) {
            Ok(count) => events.record(
                if closed {
                    "APP_SAFE_CLOSE"
                } else {
                    "APP_MINIMIZE"
                },
                format!("app={name} windows={count}"),
            ),
            Err(e) => warnings.push(format!(
                "Could not {} {name}: {e}. It was left running.",
                if closed { "close" } else { "minimize" }
            )),
        }
    }
    warnings
}

#[cfg(target_os = "windows")]
fn window_action(pids: &HashSet<u32>, close: bool) -> Result<u32, String> {
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible,
                PostMessageW, SW_MINIMIZE, ShowWindow, WM_CLOSE,
            },
        },
        core::BOOL,
    };
    struct Data<'a> {
        pids: &'a HashSet<u32>,
        close: bool,
        count: u32,
    }
    unsafe extern "system" fn callback(hwnd: HWND, param: LPARAM) -> BOOL {
        let data = unsafe { &mut *(param.0 as *mut Data) };
        let mut window_pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        }
        // Only visible, unowned top-level windows: dialogs close with their owner.
        let top_level =
            !matches!(unsafe { GetWindow(hwnd, GW_OWNER) }, Ok(owner) if !owner.is_invalid());
        if data.pids.contains(&window_pid)
            && top_level
            && unsafe { IsWindowVisible(hwnd).as_bool() }
        {
            if data.close {
                let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
            } else {
                let _ = unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
            }
            data.count += 1;
        }
        BOOL(1)
    }
    let mut data = Data {
        pids,
        close,
        count: 0,
    };
    unsafe {
        EnumWindows(Some(callback), LPARAM(&mut data as *mut _ as isize))
            .map_err(|e| e.to_string())?;
    }
    if data.count == 0 {
        Err("no visible window was found".into())
    } else {
        Ok(data.count)
    }
}
#[cfg(not(target_os = "windows"))]
fn window_action(_pids: &HashSet<u32>, _close: bool) -> Result<u32, String> {
    Err("Windows integration is only available on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, exe: &str, process: &str) -> AppResource {
        AppResource {
            id: name.into(),
            workspace_id: "w".into(),
            display_name: name.into(),
            executable_path: exe.into(),
            process_name: process.into(),
            launch_args: String::new(),
            enabled: true,
            exit_behavior_override: None,
        }
    }
    fn proc(pid: u32, name: &str, exe: &str) -> RunningProcess {
        RunningProcess {
            pid,
            name: name.into(),
            exe: Some(exe.into()),
        }
    }
    fn table() -> Vec<RunningProcess> {
        vec![
            proc(10, "Code.exe", r"C:\VSCode\Code.exe"),
            proc(11, "Code.exe", r"C:\VSCode\Code.exe"),
            proc(20, "Discord.exe", r"C:\Discord\app\Discord.exe"),
            proc(30, "chrome.exe", r"C:\Chrome\chrome.exe"),
            proc(40, "Spotify.exe", r"C:\Spotify\Spotify.exe"),
            proc(50, "WindowsTerminal.exe", r"C:\WT\WindowsTerminal.exe"),
        ]
    }

    #[test]
    fn regression_bug_018_windows_paths_in_arguments_keep_backslashes() {
        assert_eq!(
            split_args(r#"--user-data-dir C:\Users\me\Profile "C:\Program Files\x" --flag="" "#),
            vec![
                "--user-data-dir",
                r"C:\Users\me\Profile",
                r"C:\Program Files\x",
                "--flag="
            ]
        );
        assert!(split_args("   ").is_empty());
        assert_eq!(split_args(r#""""#), vec![""]);
    }

    #[test]
    fn process_detection_matches_name_or_path_case_insensitively() {
        let code = app("VS Code", r"c:\vscode\code.exe", "");
        assert_eq!(pids_for(&code, &table()), HashSet::from([10, 11]));
        let by_name = app("Discord", r"C:\Discord\Update.exe", "discord.exe");
        assert_eq!(pids_for(&by_name, &table()), HashSet::from([20]));
        assert!(pids_for(&app("Steam", r"C:\Steam\steam.exe", "steam"), &table()).is_empty());
    }

    #[test]
    fn launch_skips_running_missing_disabled_and_duplicate_apps() {
        let mut disabled = app("Disabled", r"C:\Tools\a.exe", "");
        disabled.enabled = false;
        let apps = vec![
            app("VS Code", r"C:\VSCode\Code.exe", "Code"),
            app("Steam", r"C:\Steam\steam.exe", "steam"),
            app("Steam again", r"C:\Steam\steam.exe", "steam"),
            app("Moved", r"C:\Old\moved.exe", ""),
            disabled,
        ];
        let (launch, warnings) = plan_launch(&apps, &table(), |p| !p.ends_with("moved.exe"));
        assert_eq!(
            launch
                .iter()
                .map(|a| a.display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Steam"]
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Moved"));
    }

    #[test]
    fn keep_running_does_nothing_and_minimize_targets_all_app_processes() {
        let apps = vec![app("VS Code", r"C:\VSCode\Code.exe", "Code")];
        assert!(plan_exit(&apps, "keep", &[], &table()).is_empty());
        let steps = plan_exit(&apps, "minimize", &[], &table());
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].action, ExitAction::Minimize);
        assert_eq!(steps[0].pids, HashSet::from([10, 11]));
    }

    #[test]
    fn per_app_override_wins_and_unknown_behaviors_do_nothing() {
        let mut spotify = app("Spotify", r"C:\Spotify\Spotify.exe", "");
        spotify.exit_behavior_override = Some("keep".into());
        let mut terminal = app("Terminal", r"C:\WT\WindowsTerminal.exe", "");
        terminal.exit_behavior_override = Some("kill".into());
        assert!(plan_exit(&[spotify, terminal], "safe_close", &[], &table()).is_empty());
    }

    #[test]
    fn regression_bug_007_browsers_are_never_safe_closed() {
        let apps = vec![
            app("Chrome", r"C:\Chrome\chrome.exe", ""),
            app("Discord", r"C:\Discord\app\Discord.exe", ""),
        ];
        let steps = plan_exit(&apps, "safe_close", &[], &table());
        assert_eq!(steps[0].action, ExitAction::Minimize);
        assert!(steps[0].note.as_deref().unwrap().contains("web browser"));
        assert_eq!(steps[1].action, ExitAction::SafeClose);
    }

    #[test]
    fn regression_bug_006_apps_shared_with_the_next_workspace_are_left_alone() {
        let work = vec![
            app("Discord", r"C:\Discord\app\Discord.exe", ""),
            app("VS Code", r"C:\VSCode\Code.exe", ""),
        ];
        let games = vec![app("Discord (games)", r"c:\discord\app\discord.exe", "")];
        let steps = plan_exit(&work, "safe_close", &games, &table());
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].app.display_name, "VS Code");
    }

    #[test]
    fn apps_that_are_not_running_produce_no_action_or_warning() {
        let apps = vec![app("Steam", r"C:\Steam\steam.exe", "")];
        assert!(plan_exit(&apps, "minimize", &[], &table()).is_empty());
    }

    #[test]
    fn own_process_is_never_targeted() {
        let me = std::process::id();
        let table = vec![proc(me, "app.exe", r"C:\ContextSpace\app.exe")];
        let own = app("Context Space", r"C:\ContextSpace\app.exe", "");
        assert!(pids_for(&own, &table).is_empty());
    }
}
