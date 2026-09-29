# QA 策略

QA 是開發流程的一部分，不是一次性的清理工作。每個功能或修改都必須經過：

```
實作 → 測試 → 執行完整 regression → 修正失敗 → 更新文件 → 才算完成
```

## 測試架構

系統的風險集中在元件之間的邊界，因此每一層都要能獨立測試，不能只從 UI 測：

```
Browser Extension (tabs.ts / shared.ts)
      ↓ HTTP long-poll，token 驗證
Communication Bridge (bridge.rs)
      ↓
Tauri Desktop App (commands.rs)
      ↓
Workspace Service（切換 / Session）
      ↓
SQLite (db.rs) / Windows APIs (windows.rs)
```

| 層級 | 工具 | 位置 | 測什麼 |
|---|---|---|---|
| Unit（TS） | Vitest | `apps/desktop/src/**/*.test.ts(x)`、`apps/desktop/tests/unit/` | URL 正規化、Session 去重、啟動清單、擴充功能的純邏輯（開啟計畫、關閉選擇、擷取、擁有權）、Workspace CRUD（瀏覽器 Demo 後端） |
| Unit（Rust） | `cargo test` | 各模組的 `#[cfg(test)] mod tests` | SQLite CRUD、遷移、損毀資料庫、鎖定、URL fixture、Windows 參數解析、行程比對、離開計畫 |
| Integration（TS） | Vitest + fake `chrome` + stub `fetch` | `apps/desktop/tests/integration/` | 擴充功能 ↔ bridge（真實 `tabs.ts` 程式碼、模擬瀏覽器）、UI 服務 ↔ 持久化 |
| Integration（Rust） | `cargo test` | `bridge.rs`、`commands.rs` 的測試 | 真實 HTTP bridge（token、路由、long-poll、capture）；以 **FakeExtension** 透過真實 HTTP 驅動完整切換流程 |
| Regression | Vitest / `cargo test regression_bug` | `apps/desktop/tests/regression/bug-XXX-*.test.ts(x)`、Rust `regression_bug_XXX_*` | 每個修過的 bug 一個以上，永久保留 |
| E2E | Playwright（已安裝的 Edge） | `apps/desktop/tests/e2e/*.spec.ts` | 使用者流程 1–5、錯誤訊息、UI 品質（小視窗、長名稱、大量 Workspace、主題、鍵盤、Debug 面板） |
| Manual | 檢查清單 | [MANUAL_TEST_CHECKLIST.md](MANUAL_TEST_CHECKLIST.md) | 真實瀏覽器、真實 Windows app、tray、重啟 |

### Unit / Integration / E2E 的差異

- **Unit**：單一函式或模組，沒有 I/O 或以記憶體替代（`Database::memory()`、假的行程清單）。快速、精確指出錯在哪裡。
- **Integration**：兩個以上元件透過真實介面互動。例如 Rust 測試啟動真正的 HTTP bridge（port 0），`FakeExtension` 像真的擴充功能一樣 long-poll、回傳擷取結果，然後檢查切換 Work → Games 時送出的指令順序與內容，以及 SQLite 中存下的 Session。
- **E2E**：以使用者的角度操作 UI。E2E 目前執行的是 production web build（`vite build` + `vite preview`），後端是 `localStorage` Demo，與 Tauri 使用同一套 React 程式碼與 API 介面。真實分頁 / app 的行為由上面兩層的 integration 測試負責，Tauri WebView 本身與 tray 列在手動測試。

## Browser Extension 測試方式

1. **純邏輯**集中在 `apps/extension/src/shared.ts`（`planOpen`、`selectOwnedTabIds`、`selectCaptureTabs`、`ownerForNewTab`、`normalize`），用 unit test 覆蓋所有分支。
2. **與 Chrome API 的互動**在 `apps/extension/src/tabs.ts`，由 `tests/helpers/fake-chrome.ts` 模擬：
   - 新分頁一開始是「載入中」（`url` 為空、只有 `pendingUrl`），`finishLoading()` 後才有 URL，可設定 redirect
   - `restartBrowser()` 清空 `storage.session` 並讓 tab id 從 1 重新開始
   - `stubBridge()` 模擬桌面 App：正常、離線（fetch 失敗）、401
3. **Popup** 以 jsdom 載入真實 `popup.html` 與 `popup.ts` 測試（見 `bug-023`）。
4. **權限**不能假設有效：`manifest.json` 只要求 `tabs`、`storage`、`alarms` 與 `http://127.0.0.1:47651/*`。真實瀏覽器中的權限、載入與重啟列在手動清單。

### 分頁安全規則（不可違反）

- 擴充功能只會關閉**屬於該 Workspace**的分頁。分頁取得擁有權的方式只有：
  1. 擴充功能為該 Workspace 開啟（記錄 `sourceUrl`）
  2. 由該 Workspace 的分頁開啟（繼承）
  3. 在該 Workspace 為 active、且桌面 App 可連線時新開
  4. 使用者在 popup 中把它加入 Workspace
  5. 另一個 Workspace 的分頁被這個 Workspace 開啟同一網址時轉移
- 使用者在任何 Workspace 之前就開著的分頁、瀏覽器重啟後還原的分頁：**永遠未擁有、永遠不會被關閉**。
- 擁有權存在 `chrome.storage.session`，瀏覽器重啟就清除（tab id 會重用）。
- 關閉條件 = 擁有者相符 **且**（目前 URL 或 `sourceUrl`）在關閉清單中。下一個 Workspace 也需要的網址不會被關閉。

### URL 正規化規則

唯一的規格是 `apps/desktop/tests/fixtures/url-normalization.json`，Rust、桌面 UI、擴充功能都用它測試：

- 移除：fragment（`#…`）、預設 port、沒有 query 時路徑結尾的 `/`
- 保留差異：scheme（http ≠ https）、`www` 與否、路徑大小寫、query、不同路徑（`/jobs` ≠ `/messages`）
- 只接受 http / https
- 不做網域層級的比對：`linkedin.com` 永遠不會比對到 `linkedin.com/jobs`

## Windows App 測試方式

- `windows.rs` 把決策與執行分開：`plan_launch`、`plan_exit` 接收行程清單（`RunningProcess`）並回傳計畫，單元測試用假的行程清單驗證 VS Code、Discord、Steam、Spotify、瀏覽器、Terminal 等情境。
- 真正的 Win32 呼叫（`EnumWindows`、`ShowWindow(SW_MINIMIZE)`、`PostMessageW(WM_CLOSE)`）只在 `window_action`，需人工驗證。
- 規則：
  - **絕不**終止行程（沒有 `TerminateProcess`、`taskkill`、`kill`）
  - Keep Running：不做任何事
  - Minimize：最小化所有可見的頂層視窗，行程保持執行
  - Safely Close：送出 `WM_CLOSE`，讓 app 自己詢問是否儲存；失敗時顯示警告，行程保持執行
  - 瀏覽器一律不安全關閉（改為最小化）；Context Space 自己永遠不會被操作
  - 下一個 Workspace 也包含的 app 不處理
  - 已在執行的 app 不重複啟動；執行檔遺失時略過並警告

## Regression 政策

1. 先重現 bug。
2. 寫一個**因為這個 bug 而失敗**的測試（TS：`tests/regression/bug-XXX-簡短描述.test.ts`；Rust：`fn regression_bug_XXX_描述()`）。
3. 修復。
4. 執行測試，確認通過。
5. **永遠保留**這個測試。刪除 regression test 需要在 PR 中說明理由。

本次穩定化時用「突變測試」驗證了 regression test 的有效性：把修復暫時還原，確認對應測試會失敗（TS：BUG-003/005/014/015；Rust：BUG-004/006/007/009）。

新增功能時，必須同時執行**整個** regression suite，而不只是新功能的測試。

## Bug 嚴重度

| 等級 | 範例 | 發佈 |
|---|---|---|
| Critical | 當機、資料遺失、Workspace 設定損毀、SQLite 損毀、誤關分頁、強制結束 app、未儲存的工作遺失 | 必須修復 |
| High | 切換失敗、分頁無法還原、app 無法啟動、擴充功能無法與桌面通訊、關錯資源、重複開啟分頁/app | 必須修復 |
| Medium | UI 未更新、active 標示錯誤、tray 過時、順序錯誤 | 可延後，需記錄 |
| Low | 動畫、間距、外觀 | 可延後 |

## Release gate

下列任一條件成立就**不得發佈**：

- 有未修復的 Critical bug
- 有未解決的 High 等級 regression
- `npm run qa` 失敗
- `npm run test:e2e` 失敗（或未記錄原因）
- `npm run build`（Tauri production build）失敗
- 手動檢查清單的「發佈前必測」未完成

PR 必須通過 CI（`.github/workflows/ci.yml`）並完成 `.github/pull_request_template.md` 的 QA checklist。

## 指令

| 指令 | 內容 |
|---|---|
| `npm run format:check` | `cargo fmt --check` |
| `npm run lint` | oxlint（桌面 + 擴充功能，警告視為錯誤）+ `cargo clippy -D warnings` |
| `npm run typecheck` | 桌面與擴充功能 `tsc` |
| `npm run test:unit` | Vitest unit |
| `npm run test:integration` | Vitest integration |
| `npm run test:regression` | Vitest regression + `cargo test regression_bug` |
| `npm run test:rust` | 所有 Rust 測試（unit、SQLite、bridge、切換整合） |
| `npm test` | unit + integration + regression + Rust |
| `npm run test:e2e` | Playwright（需要已安裝 Microsoft Edge） |
| `npm run test:all` | `npm test` + E2E |
| `npm run qa` | format → lint → typecheck → 全部測試 → web/擴充功能 build |
| `npm run qa:full` | `qa` + E2E |
| `npm run build` | 擴充功能 + Tauri production build（NSIS） |

Windows 本機需要 `cargo` 在 PATH（`%USERPROFILE%\.cargo\bin`）。

## 如何加入新的測試

1. **先決定層級**：能用純函式測就寫 unit；跨越邊界（擴充功能 ↔ bridge、切換 ↔ SQLite）就寫 integration；使用者可見的流程再加 E2E。
2. **TypeScript**：放在 `apps/desktop/tests/{unit,integration,regression}/`。擴充功能程式碼直接 import `../../../extension/src/...`，需要瀏覽器時用 `installFakeChrome()`，需要桌面 App 時用 `stubBridge()`。
3. **Rust**：寫在對應模組的 `mod tests`。資料庫用 `Database::memory()`；需要真實檔案用 `temp_path()`；需要擴充功能用 `commands::tests::FakeExtension`。
4. **E2E**：放在 `apps/desktop/tests/e2e/`，使用 `helpers.ts` 的 `start(page, seed?)`（只在第一次載入清空資料）與精確的 selector（`exact: true`）。
5. **Schema 變更**：在 `db.rs` 的 `MIGRATIONS` 最後**新增**一步（不可修改既有步驟、不可重建資料表），並加一個「舊版資料庫升級後資料仍在」的測試。
6. 執行 `npm run qa:full`，更新本文件或 README。

## 未來功能的測試重點

| 功能 | 必須同時驗證 |
|---|---|
| Snapshots | Last Session 仍正常儲存與還原 |
| Focus Mode | 不會關閉任何未擁有或其他 Workspace 的分頁 |
| 視窗位置還原 / 多螢幕 | app 啟動、最小化、安全關閉仍正常 |
| Firefox | Chrome 與 Edge 整合不受影響；先解決 BUG-028 |
| Cloud Sync | 離線時完全可用（local-first） |
| AI 分類 | 未經使用者明確同意，不得自動關閉或移動任何資源 |

實作前先寫下預期行為、列出可能受影響的既有功能、先加入測試案例。
