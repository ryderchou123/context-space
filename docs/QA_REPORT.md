# QA 報告：原型穩定化

本報告只列出**實際執行過**的結果。沒有執行的項目明確標示為「未執行」。

## 測試日期

2026-09-28

## 測試環境

| 項目 | 版本 |
|---|---|
| 作業系統 | Windows 11 家用版 10.0.26200 |
| Node.js | v24.14.1 |
| Rust | rustc / cargo 1.98.1 |
| Vitest | 5.0.2（jsdom） |
| Playwright | 1.63.0，使用已安裝的 Microsoft Edge 154.0.4258.37 |
| Google Chrome | 154.0.8037.57（已安裝；未用於自動化測試） |
| Tauri | 2.x（`tauri-plugin-single-instance` 2.5.0） |

## 測試範圍

- 桌面 App：SQLite 資料層與遷移、bridge HTTP 伺服器、Workspace 切換 / 關閉 / Session 還原、Windows app 啟動與離開計畫、事件記錄
- 瀏覽器擴充功能：分頁擁有權、開啟 / 關閉 / 擷取、popup、離線與 token 錯誤
- React UI：Workspace 與資源管理、編輯器、切換、Session 還原、刪除、Debug 面板、無障礙、版面
- 跨邊界：擴充功能 ↔ bridge（TS，模擬瀏覽器）、bridge ↔ 切換服務 ↔ SQLite（Rust，透過真實 HTTP 的 FakeExtension）

## Bug 摘要

詳細內容見 [QA_AUDIT.md](QA_AUDIT.md)。

| 嚴重度 | 發現 | 已修復 | 部分修復 | 開放 |
|---|---|---|---|---|
| Critical | 4 | 4 | 0 | 0 |
| High | 11 | 11 | 0 | 0 |
| Medium | 13 | 11 | 1（BUG-027） | 1（BUG-028） |
| Low | 5 | 5 | 0 | 0 |
| **合計** | **33** | **31** | **1** | **1** |

### 已修復

BUG-001、002、003、004、005、006、007、008、009、010、011、012、013、014、015、016、017、018、019、020、021、022、023、024、025、026、029、030、031、032、033。其中 BUG-016、021、022 的修復**只能人工驗證**（UI 執行緒、tray、單一實例），本次尚未人工驗證，見「已知限制」。

### 尚未修復

- **BUG-027（Medium，部分修復）**：擴充功能離線時，網址以預設瀏覽器開啟，無法偵測重複；目前會顯示明確警告。
- **BUG-028（Medium，開放）**：Chrome 與 Edge 同時安裝擴充功能時共用同一個指令佇列。暫行做法：只在一個瀏覽器安裝（已寫入 README）。

Release gate：Critical = 0、未解決的 High regression = 0 → **自動化部分符合發佈條件**；發佈前仍需完成手動清單。

## 基準結果（修改前）

| 檢查 | 結果 |
|---|---|
| `cargo test` | 9/9 通過 |
| `cargo fmt --check` | 失敗（11 處差異） |
| `cargo clippy` | 5 個警告 |
| typecheck / oxlint | 通過 |
| Vitest unit / regression | 9/9、2/2 通過 |
| Vitest integration | **0/2 通過** |
| Playwright E2E | **1/4 通過** |
| `npm run qa` | **失敗**（第一步 format check） |

## 修改後結果

以下皆為本機實際執行結果（`cargo` 需在 PATH，本機以 `%USERPROFILE%\.cargo\bin` 執行）。

### `npm run qa`：通過（exit 0，69 秒）

依序執行，全部通過：

| 步驟 | 結果 |
|---|---|
| `format:check`（cargo fmt） | 通過 |
| `lint`：oxlint 桌面（`--deny-warnings`） | 0 警告 |
| `lint`：oxlint 擴充功能（`--deny-warnings`） | 0 警告 |
| `lint:rust`：`cargo clippy --all-targets -D warnings` | 0 警告 |
| `typecheck`：桌面 + 擴充功能 | 通過 |
| `test:unit` | **84/84 通過**（6 個檔案） |
| `test:integration` | **15/15 通過**（2 個檔案） |
| `test:regression`（Vitest） | **20/20 通過**（12 個檔案） |
| `test:regression`（Rust `regression_bug`） | **15/15 通過** |
| `test:rust`（全部 Rust 測試） | **51/51 通過** |
| `build:web`（桌面 web build + 擴充功能 build） | 成功 |

### Unit test

- Vitest：84 個，包含 URL 正規化（以共用 fixture 驗證兩份 TS 實作）、Session、擴充功能純邏輯、Workspace / 瀏覽器資源 / app 資源 CRUD
- Rust unit（包含在 51 個之中）：SQLite CRUD、排序、驗證、URL fixture、Windows 參數解析、行程比對、啟動 / 離開計畫、事件記錄

### Integration test

- Vitest 15 個：擴充功能 `tabs.ts` 在模擬瀏覽器中的開啟 / 去重 / Work→Games / 擁有權繼承與轉移 / 離線 / 401 / token；UI 服務 ↔ 持久化
- Rust：真實 HTTP bridge（token、狀態、404、錯誤 JSON、capture round-trip、延遲擷取、add-tabs 重複統計）；FakeExtension 驅動 Flow 3、Flow 4、共用網址、Keep tabs、擷取逾時、離線、重啟復原、忙碌鎖、12 執行緒同時切換、快速循環切換

### E2E（`npm run test:e2e`）：21/21 通過

連續執行兩次皆為 21/21（40.2 秒、1.1 分鐘；第二次與 Tauri release build 同時進行）。

| 檔案 | 內容 |
|---|---|
| `workspace-create.spec.ts` | Flow 1（建立 → 重新載入後仍在）、空白名稱、改名後設定保留 |
| `workspace-resources.spec.ts` | Flow 2（加入 LinkedIn → 重新開啟 / 重新載入後仍在）、重複網址、不合法網址、BUG-014（等待 5.6 秒後輸入仍在） |
| `workspace-switch.spec.ts` | Flow 3（UI 部分）、雙擊與快速切換後只有一個 active、關閉 Workspace |
| `session-restore.spec.ts` | Flow 4（UI 部分）、空 Session 不顯示還原 |
| `workspace-delete.spec.ts` | Flow 5（刪除後重新載入不存在、無殘留資料）、取消刪除 |
| `ui-quality.spec.ts` | 900×640 無水平捲動、長名稱、40 個 Workspace、主題切換並記住、鍵盤操作、所有按鈕有名稱、Debug 面板 |

注意：E2E 使用 production web build 與 `localStorage` 後端。Flow 3、Flow 4 中「分頁被擷取 / 關閉 / 還原、app 最小化」的部分**由 Rust integration test 驗證**，不是由瀏覽器 E2E 驗證。

### Regression test

- Vitest 12 個檔案 / 20 個測試：BUG-001、002、003、005、008、014、015、020、023、025、031、032、033
- Rust 15 個 `regression_bug_*`：BUG-004、006（×2）、007、009（×2）、010（×2）、011、012、013、017（×2）、018、029
- 有效性驗證（突變測試）：暫時把修復還原後，對應測試確實失敗：
  - TS：BUG-003、BUG-005（2 個測試）、BUG-014（2 個）、BUG-015 → 6 失敗
  - Rust：BUG-004、006、007、009（2 個）→ 5 失敗
  - 還原修復後全部重新通過

### Build status

| Build | 結果 |
|---|---|
| `npm run build:web` | 成功 |
| 擴充功能 `npm run build` | 成功（`apps/extension/dist`，manifest 0.1.1） |
| `npm run build`（Tauri release + NSIS） | **成功**，4 分 7 秒編譯；產出 `Context Space_0.1.0_x64-setup.exe`（3.10 MiB） |
| GitHub Actions（Windows） | **成功**：[PR #10 / CI run 36513715105](https://github.com/ryderchou123/context-space/actions/runs/36513715105)；install、format、lint、typecheck、unit、integration、regression、Rust、web/extension build、Playwright E2E、Tauri/NSIS build 全部通過 |

### 未執行的項目

- **手動測試清單**：未執行。本機已有一個正在執行的 Context Space（占用 bridge port 47651）與真實資料庫；為避免修改使用者資料，沒有啟動新版執行檔。
- **真實 Chrome / Edge 載入擴充功能**：未執行（以模擬瀏覽器測試）。
- **真實 Win32 最小化 / WM_CLOSE**：未執行（以行程清單模擬測試決策邏輯）。
- **GitHub labels / issues**：QA 專用 labels 與 BUG-027／BUG-028 issues 尚未建立；GitHub CLI 已安裝但目前登入 token 失效。已提供 `scripts/create-github-labels.ps1` 與 `.github/ISSUE_TEMPLATE/bug_report.md`。

## 已知限制

1. 真實瀏覽器、真實 Windows app、tray、單一實例（BUG-016、021、022）只能人工驗證，本次尚未驗證。
2. E2E 沒有在 Tauri WebView 中執行（沒有使用 `tauri-driver`）。
3. 分頁擁有權只存在於單次瀏覽器執行期間。瀏覽器重啟後還原的分頁不會被關閉，也不會被擷取（刻意選擇安全的一方）。
4. 擁有權規則：Workspace 為 active 時新開的分頁會歸屬該 Workspace，離開時（設定為 Close Tabs）會被關閉。這是 Flow 4 需要的行為，但使用者需要知道。
5. 「安全關閉」只送出 `WM_CLOSE`，無法確認 app 真的結束（app 可能詢問是否儲存），這是刻意的設計。
6. 透過啟動器執行的 app（例如 Discord 的 `Update.exe`）需要填寫正確的行程名稱，否則每次都會嘗試啟動。
7. Microsoft Store / UWP app 的行程偵測未測試。
8. `apps/desktop/tests/` 下的測試檔不經過 `tsc` 型別檢查（Vitest 只做轉譯）。
9. BUG-027、BUG-028 尚未完全修復（見上方）。
