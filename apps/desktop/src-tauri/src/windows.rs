use crate::models::AppResource;
use std::{path::Path, process::Command};
use sysinfo::System;

pub fn launch_apps(apps: &[AppResource]) -> Vec<String> {
    let mut warnings = vec![];
    for app in apps.iter().filter(|a| a.enabled) {
        if !Path::new(&app.executable_path).exists() {
            warnings.push(format!(
                "{} was skipped because its executable is missing.",
                app.display_name
            ));
            continue;
        }
        if !process_ids(app).is_empty() {
            continue;
        }
        let mut command = Command::new(&app.executable_path);
        if let Some(args) = shlex::split(&app.launch_args) {
            command.args(args);
        }
        match command.spawn() {
            Ok(_) => {}
            Err(e) => warnings.push(format!("Could not open {}: {e}", app.display_name)),
        }
    }
    warnings
}

pub fn apply_exit(apps: &[AppResource], default_behavior: &str) -> Vec<String> {
    let mut warnings = vec![];
    for app in apps.iter().filter(|a| a.enabled) {
        let behavior = app
            .exit_behavior_override
            .as_deref()
            .unwrap_or(default_behavior);
        if behavior == "keep" {
            continue;
        }
        for pid in process_ids(app) {
            if let Err(e) = window_action(pid, behavior == "safe_close") {
                warnings.push(format!(
                    "Could not {} {}: {e}",
                    if behavior == "safe_close" {
                        "close"
                    } else {
                        "minimize"
                    },
                    app.display_name
                ));
            }
        }
    }
    warnings
}

fn process_ids(app: &AppResource) -> Vec<u32> {
    let mut system = System::new_all();
    system.refresh_all();
    let wanted_name = app.process_name.trim_end_matches(".exe");
    let wanted_path = Path::new(&app.executable_path);
    system
        .processes()
        .values()
        .filter(|p| {
            let name = p.name().to_string_lossy();
            let name_match = !wanted_name.is_empty()
                && name
                    .trim_end_matches(".exe")
                    .eq_ignore_ascii_case(wanted_name);
            let path_match = p.exe().is_some_and(|p| same_path(p, wanted_path));
            name_match || path_match
        })
        .map(|p| p.pid().as_u32())
        .collect()
}
fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}

#[cfg(target_os = "windows")]
fn window_action(pid: u32, close: bool) -> Result<(), String> {
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, SW_MINIMIZE,
                ShowWindow, WM_CLOSE,
            },
        },
        core::BOOL,
    };
    struct Data {
        pid: u32,
        close: bool,
        count: u32,
    }
    unsafe extern "system" fn callback(hwnd: HWND, param: LPARAM) -> BOOL {
        let data = unsafe { &mut *(param.0 as *mut Data) };
        let mut window_pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        }
        if window_pid == data.pid && unsafe { IsWindowVisible(hwnd).as_bool() } {
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
        pid,
        close,
        count: 0,
    };
    unsafe {
        EnumWindows(Some(callback), LPARAM(&mut data as *mut _ as isize))
            .map_err(|e| e.to_string())?;
    }
    if data.count == 0 {
        Err("no visible top-level window was found".into())
    } else {
        Ok(())
    }
}
#[cfg(not(target_os = "windows"))]
fn window_action(_pid: u32, _close: bool) -> Result<(), String> {
    Err("Windows integration is only available on Windows.".into())
}
