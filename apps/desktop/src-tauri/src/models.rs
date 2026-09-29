use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserResource {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub url: String,
    pub pinned: bool,
    pub enabled: bool,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppResource {
    pub id: String,
    pub workspace_id: String,
    pub display_name: String,
    pub executable_path: String,
    pub process_name: String,
    pub launch_args: String,
    pub enabled: bool,
    pub exit_behavior_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTab {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub position: i64,
    pub browser_exit_behavior: String,
    pub app_exit_behavior: String,
    pub save_session_on_exit: bool,
    pub restore_session_on_launch: bool,
    pub browser_resources: Vec<BrowserResource>,
    pub app_resources: Vec<AppResource>,
    pub last_session: Vec<SessionTab>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDraft {
    pub id: Option<String>,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub browser_exit_behavior: String,
    pub app_exit_behavior: String,
    pub save_session_on_exit: bool,
    pub restore_session_on_launch: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub workspaces: Vec<Workspace>,
    pub active_workspace_id: Option<String>,
    pub active_workspace_recovered: bool,
    pub extension_connected: bool,
    pub bridge_token: String,
    pub bridge_port: u16,
}

#[derive(Debug, Serialize)]
pub struct OperationResult {
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeCommand {
    CaptureTabs {
        request_id: String,
        workspace_id: String,
    },
    OpenUrls {
        workspace_id: String,
        urls: Vec<String>,
    },
    CloseUrls {
        workspace_id: String,
        urls: Vec<String>,
    },
}

impl BridgeCommand {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::CaptureTabs { .. } => "capture_tabs",
            Self::OpenUrls { .. } => "open_urls",
            Self::CloseUrls { .. } => "close_urls",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedApp {
    pub display_name: String,
    pub running: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugInfo {
    pub active_workspace_id: Option<String>,
    pub active_workspace_name: Option<String>,
    pub active_workspace_recovered: bool,
    pub extension_connected: bool,
    pub extension_last_seen_secs: Option<u64>,
    pub database_path: String,
    pub database_status: String,
    pub schema_version: i64,
    pub last_session_tabs: usize,
    pub tracked_apps: Vec<TrackedApp>,
    pub last_transition: Option<crate::events::LogEntry>,
    pub recent_events: Vec<crate::events::LogEntry>,
}
