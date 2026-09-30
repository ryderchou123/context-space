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

所有切換、關閉、還原、刪除共用一把 `try_lock`：同時間只允許一個操作，其餘立即回傳「still running」，不排隊。這些 command 以 async 執行，不占用主執行緒。

1. 讀取目標 Workspace 與 active Workspace；active 指向已不存在的 Workspace 時直接略過離開步驟。
2. 若 active 是「重新啟動後復原」的狀態（`active_workspace_recovered`），**不做任何離開動作**，只顯示說明。
3. 若開啟 save-session，要求 extension capture 該 Workspace 擁有的分頁；2.6 秒 timeout 或擷取結果為空時保留舊 session。
4. 重新讀取 Workspace（含剛存的 session），扣除目標 Workspace 也需要的網址後 queue close-tabs。
5. 扣除目標 Workspace 也包含的 app 後，minimize／gracefully close apps（瀏覽器永遠只最小化）。
6. 合併目標 Workspace 的 pinned URLs 與選擇性 Last Session，以 normalized URL 去重。
7. Extension 在線時 queue open-URLs；否則交給 Windows default browser 並警告無法去重。
8. 以一次 process 快照檢查後啟動尚未執行的 applications。
9. 寫入 `active_workspace_id`、清除 recovered 旗標、刷新 tray 並發出 `workspace-changed` 事件。

## Database schema

- `workspaces`：名稱、icon、color、排序、browser/app exit behavior、session flags、timestamps。
- `browser_resources`：Workspace FK、title、normalized URL、pinned、enabled、position；`workspace_id + url` 唯一。
- `app_resources`：Workspace FK、display name、executable path、process name、arguments、enabled、optional behavior override。
- `workspace_sessions`：每次保存的 session metadata；V1 只保留最新一份。
- `session_tabs`：session FK、title、URL、position。
- `app_state`：active Workspace、`active_workspace_recovered`、bridge token。

Schema 版本存在 `PRAGMA user_version`，`db.rs` 的 `MIGRATIONS` 依序升級，每一步與版本號在同一個 transaction 內提交。開啟時先 `PRAGMA quick_check`；損毀或版本較新時拒絕開啟且不修改檔案。

## Browser bridge

Bridge 固定監聽 `127.0.0.1:47651`，不綁定 LAN interface。除了 CORS preflight 外，每個 endpoint 都驗證 `X-Context-Space-Token`。MV3 worker 在 `chrome.storage.local` 保存 durable token 與每次安裝獨立的 client ID，並以 `X-Context-Space-Client` 傳送。Bridge 為每個在線 client 維護獨立 command queue；Chrome 與 Edge 同時連線時都會收到切換命令，capture responses 會以正規化 URL 合併去重。沒有 client header 的舊版擴充功能會暫時歸入 `legacy` client。Command queue 不把瀏覽資料送往網際網路；佇列中的命令 20 秒後過期，避免擴充功能重新連線時執行過時的關閉命令。MV3 worker 使用 alarms 喚醒並處理命令。

### 分頁擁有權

擴充功能在 `chrome.storage.session` 記錄 `tabId → { workspaceId, sourceUrl }`（瀏覽器重啟即清除，因為 tab id 會重用）。擷取與關閉只作用在擁有者相符的分頁；關閉還需要目前 URL 或 `sourceUrl` 在清單中（`sourceUrl` 讓重新導向後的分頁仍可辨識）。新分頁繼承開啟者的擁有者，否則歸屬 active Workspace；使用者自己既有的分頁可被重用但不會被收編。URL 正規化規則以 `apps/desktop/tests/fixtures/url-normalization.json` 為準。

## Windows safety

Process matching 使用 executable path 或 configured process name（不分大小寫），且排除 Context Space 自己。`plan_launch`／`plan_exit` 是純函式，Win32 呼叫集中在 `window_action`。Exit adapter 一次處理 app 所有 PID 的 visible、unowned top-level windows。Minimize 呼叫 `ShowWindow(SW_MINIMIZE)`；safe close 發送 `WM_CLOSE`。系統不呼叫 `TerminateProcess`。

## Cross-platform extension point

資料模型與切換 orchestration 不依賴 Win32；`windows.rs` 是獨立 platform adapter。未來可為 macOS／Linux 實作相同的 launch、minimize 與 graceful-close interface。
