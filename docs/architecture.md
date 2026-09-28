# Context Space 架構

## 邊界

React renderer 只處理 UI 與輸入驗證；privileged operation 全部透過明確的 Tauri command 進入 Rust。Rust 擁有 SQLite、檔案存在性檢查、process discovery、Win32 window action、system tray 與 browser bridge。

```text
React UI ──Tauri IPC──> Rust command layer ──> SQLite
                              │                  │
                              ├──> Win32 adapter │
                              ├──> process launch│
                              └──> localhost bridge <──token── MV3 extension
```

## Workspace switch transaction

1. 讀取 active Workspace。
2. 若開啟 save-session，要求 extension capture browser tabs；2.6 秒 timeout 後保留舊 session。
3. 依離開 Workspace 的設定 queue close-tabs 並 minimize／gracefully close apps。
4. 合併目標 Workspace 的 pinned URLs 與選擇性 Last Session，以 normalized URL 去重。
5. Extension 在線時 queue open-URLs；否則交給 Windows default browser。
6. 檢查 process 後啟動尚未執行的 applications。
7. 寫入 `active_workspace_id` 並刷新 tray。

## Database schema

- `workspaces`：名稱、icon、color、排序、browser/app exit behavior、session flags、timestamps。
- `browser_resources`：Workspace FK、title、normalized URL、pinned、enabled、position；`workspace_id + url` 唯一。
- `app_resources`：Workspace FK、display name、executable path、process name、arguments、enabled、optional behavior override。
- `workspace_sessions`：每次保存的 session metadata；V1 只保留最新一份。
- `session_tabs`：session FK、title、URL、position。
- `app_state`：active Workspace 與 bridge token。

## Browser bridge

Bridge 固定監聽 `127.0.0.1:47651`，不綁定 LAN interface。除了 CORS preflight 外，每個 endpoint 都驗證 `X-Context-Space-Token`。Command queue 不把瀏覽資料送往網際網路。MV3 worker 將 durable token 放在 `chrome.storage.local`，使用 alarms 喚醒並處理命令。

## Windows safety

Process matching 優先使用 executable path，並支援 configured process name。Exit adapter 只處理屬於目標 PID 的 visible top-level windows。Minimize 呼叫 `ShowWindow(SW_MINIMIZE)`；safe close 發送 `WM_CLOSE`。系統不呼叫 `TerminateProcess`。

## Cross-platform extension point

資料模型與切換 orchestration 不依賴 Win32；`windows.rs` 是獨立 platform adapter。未來可為 macOS／Linux 實作相同的 launch、minimize 與 graceful-close interface。
