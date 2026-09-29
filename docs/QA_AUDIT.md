# QA 稽核報告（原型穩定化）

- 稽核日期：2026-09-28
- 稽核對象：`main` @ `637e00d` 加上稽核開始時工作目錄中尚未提交的修改（擴充功能分頁擁有權、切換鎖、E2E 骨架等）
- 稽核方式：執行全部既有檢查、逐檔閱讀 Rust / TypeScript / 擴充功能原始碼、以測試重現每個問題

嚴重度定義見 [QA_STRATEGY.md](QA_STRATEGY.md#bug-嚴重度)。每個已修復的 bug 都有永久保留的 regression test；Rust 測試名稱為 `regression_bug_XXX_*`，TypeScript 測試位於 `apps/desktop/tests/regression/bug-XXX-*.test.ts(x)`。

## 1. 稽核當下的基準結果（修改前）

| 檢查 | 指令 | 結果 |
|---|---|---|
| Rust 測試 | `cargo test` | 9/9 通過 |
| Rust 格式 | `cargo fmt --check` | **失敗**：11 處格式差異 → `npm run qa` 第一步就失敗 |
| Rust lint | `cargo clippy --all-targets` | 5 個警告（collapsible `if`） |
| TypeScript typecheck | `npm run typecheck` | 通過 |
| oxlint | `npm run lint` | 通過（擴充功能沒有 lint） |
| Vitest unit | `npm run test:unit` | 9/9 通過 |
| Vitest integration | `npm run test:integration` | **2/2 失敗**（BUG-015） |
| Vitest regression | `npm run test:regression` | 2/2 通過 |
| Playwright E2E | `npx playwright test` | **1/4 通過**（BUG-026：測試本身的缺陷） |
| `npm test`（apps/desktop） | `vitest run` | **失敗**：Vitest 會載入 Playwright spec |
| 環境 | — | `cargo` 不在 PATH（位於 `%USERPROFILE%\.cargo\bin`）；未安裝 `gh` CLI |

Console / Tauri / 擴充功能 / SQLite 錯誤：原型沒有結構化記錄，也沒有任何地方能看到 bridge 或資料庫狀態，因此改以程式碼審查與測試重現。這次加入了事件記錄與 Debug 面板（見第 4 節）。

## 2. Bug 清單

狀態：**已修復** = 已修復並有 regression test；**開放** = 尚未修復（已知限制）。

| ID | 嚴重度 | 區域 | 問題摘要 | 狀態 |
|---|---|---|---|---|
| BUG-001 | Critical | 擴充功能 | 離開 Workspace 時關閉所有 URL 相符的分頁，包含使用者自己開的與其他 Workspace 的 | 已修復 |
| BUG-002 | High | 擴充功能 | Session 擷取會把所有視窗的所有分頁存成該 Workspace 的 Last Session | 已修復 |
| BUG-003 | Critical | 擴充功能 | 分頁擁有權以 tab id 永久保存；瀏覽器重啟後 id 重用，可能誤關無關分頁 | 已修復 |
| BUG-004 | High | Bridge | 佇列中的指令沒有期限；擴充功能重新連線時執行數小時前的「關閉分頁」 | 已修復 |
| BUG-005 | High | 擴充功能 | 快速切換 / 「Open again」產生重複分頁 | 已修復 |
| BUG-006 | High | 切換 | 兩個 Workspace 共用的 app 或網址在切換時被關閉/最小化 | 已修復 |
| BUG-007 | Critical | Windows | 瀏覽器被設為 app 且選「安全關閉」時，所有瀏覽器視窗被關閉 | 已修復 |
| BUG-008 | High | 切換 / Session | Workspace 期間新開的分頁不會被存入 Session；關閉清單使用舊 Session | 已修復 |
| BUG-009 | High | 資料 | 擷取到 0 個分頁時覆寫 Last Session（資料遺失） | 已修復 |
| BUG-010 | High | 復原 | App 重啟後依過期的 active 狀態，下次切換會關分頁、關閉 app | 已修復 |
| BUG-011 | Medium | 資料 | 刪除 Workspace 後排序按鈕失效 | 已修復 |
| BUG-012 | Medium | 資料 | 在已刪除 Workspace 的編輯器按儲存會讓它復活 | 已修復 |
| BUG-013 | Critical | 資料 | 資料庫損毀或版本較新時 App 直接 panic，沒有任何訊息 | 已修復 |
| BUG-014 | High | UI | 編輯器每 5 秒被重置：未儲存輸入消失、分頁跳回 General | 已修復 |
| BUG-015 | Medium | UI / 測試 | 瀏覽器 Demo API 同步 throw 而非回傳 rejected promise | 已修復 |
| BUG-016 | Medium | 效能 | 切換在主執行緒執行，視窗與 tray 凍結數秒 | 已修復（手動驗證） |
| BUG-017 | High | 競態 | 切換鎖會排隊；刪除不受保護；active 指向不存在的 Workspace 時全部切換失敗 | 已修復 |
| BUG-018 | Medium | Windows | 啟動參數以 POSIX 規則解析，Windows 路徑的反斜線被刪除 | 已修復 |
| BUG-019 | Medium | URL | Rust、桌面 UI、擴充功能三處 URL 正規化規則不一致 | 已修復 |
| BUG-020 | High | 擴充功能 | 會重新導向的網址每次切換都再開一個，也關不掉 | 已修復 |
| BUG-021 | Medium | Tray / UI | 排序後 tray 未更新；從 tray 切換後儀表板最多 5 秒才更新 | 已修復（手動驗證） |
| BUG-022 | High | Tray | 可同時執行兩個 App：兩個 tray icon、bridge port 衝突 | 已修復（手動驗證） |
| BUG-023 | Medium | 擴充功能 | 桌面 App 未執行時 popup 要求重新輸入 token；有重複時仍顯示「已加入 N 個」 | 已修復 |
| BUG-024 | Low | UI | 建立/刪除失敗時對話框仍然關閉，輸入遺失 | 已修復 |
| BUG-025 | Low | UI | Toast 計時器互相干擾，新訊息被提早關閉 | 已修復 |
| BUG-026 | Medium | QA | QA gate 無法通過（fmt、clippy、Vitest 載入 E2E、E2E 測試缺陷） | 已修復 |
| BUG-027 | Medium | 切換 | 擴充功能離線時每次切換都用預設瀏覽器重開網址，無法去重 | 部分修復（有明確警告；去重需要擴充功能） |
| BUG-028 | Medium | 擴充功能 | Chrome 與 Edge 同時安裝擴充功能時共用一個指令佇列 | 開放 |
| BUG-029 | Low | Bridge | 讀不到 token 時 bridge 會接受空 token | 已修復 |
| BUG-030 | Low | Windows | 多行程 app（VS Code 等）最小化時，每個沒有視窗的子行程各產生一條警告 | 已修復 |
| BUG-031 | Medium | 擴充功能 | 同時開多個分頁時擁有權寫入互相覆蓋而遺失 | 已修復 |
| BUG-032 | Low | UI | `localhost:3000` 被當成 URL scheme 而被拒絕 | 已修復 |
| BUG-033 | Medium | 無障礙 | 編輯器開啟時錯誤訊息對螢幕閱讀器不可見 | 已修復 |

統計：Critical 4（全部修復）、High 11（全部修復）、Medium 13（11 修復、1 部分、1 開放）、Low 5（全部修復）。

## 3. Bug 詳細內容

### BUG-001 — 關閉了不屬於 Workspace 的分頁
- 嚴重度：Critical（分頁被誤關 = 使用者資料遺失）
- 重現步驟：
  1. 自己開啟 `https://linkedin.com`
  2. Work 釘選 `linkedin.com`，瀏覽器分頁設定為「Close Tabs」
  3. 開啟 Work，再切換到 Games
- 預期：只關閉 Work 開啟的分頁。
- 實際：所有 URL 相符的分頁都被關閉，包含使用者原本自己開的與 Games 的。
- 根本原因：`closeUrls` 只比對 URL，沒有任何擁有權概念（`637e00d` 版本）。
- 修復：擴充功能記錄每個分頁屬於哪個 Workspace，只關閉「屬於該 Workspace 且 URL 相符」的分頁。稽核開始前的未提交修改已加入雛形，本次補齊並改為 `{ workspaceId, sourceUrl }`。
- 測試：`tests/regression/bug-001-tab-safety.test.ts`、`tests/unit/extension-planning.test.ts`
- 狀態：已修復

### BUG-002 — Session 擷取包含所有分頁
- 嚴重度：High
- 重現步驟：開著網銀與 Twitch，開啟 Work 後切換到 Games，再看 Work 的 Last Session。
- 預期：只包含 Work 的分頁。
- 實際：所有視窗的所有 http(s) 分頁都被存入，還原時全部重開。
- 根本原因：`capture_tabs` 直接 `chrome.tabs.query({})`。
- 測試：`tests/regression/bug-002-capture-only-workspace-tabs.test.ts`
- 狀態：已修復

### BUG-003 — 瀏覽器重啟後分頁擁有權錯亂
- 嚴重度：Critical
- 重現步驟：
  1. 開啟 Work（擴充功能開了 id=1 的分頁並記錄為 Work）
  2. 關閉並重新開啟瀏覽器；使用者新開的分頁也拿到 id=1
  3. 切換 Work → Games
- 預期：使用者的分頁不受影響。
- 實際：擁有權存在 `chrome.storage.local`（永久），id=1 仍被視為 Work 的分頁；URL 相符時會被關閉，也會被擷取進 Work 的 Session。
- 根本原因：Chrome 的 tab id 只在單次瀏覽器執行期間唯一。
- 修復：擁有權改存 `chrome.storage.session`（瀏覽器重啟即清除）；更新時移除舊的永久資料。重啟後的分頁一律視為未擁有，也就是永遠不會被關閉。
- 測試：`tests/regression/bug-003-ownership-after-browser-restart.test.ts`
- 狀態：已修復

### BUG-004 — 過時的 bridge 指令被延後執行
- 嚴重度：High
- 重現步驟：擴充功能剛斷線（45 秒內仍被視為已連線）時切換 Work → Games；數小時後重新開啟瀏覽器。
- 預期：過時的指令不執行。
- 實際：`CloseUrls` 一直留在佇列中，擴充功能下次輪詢時立刻關閉分頁。
- 修復：佇列中的指令有 20 秒期限，過期即丟棄並記錄 `BRIDGE_COMMAND_EXPIRED`。
- 測試：`bridge::tests::regression_bug_004_expired_commands_are_never_delivered`
- 狀態：已修復

### BUG-005 — 快速切換造成重複分頁
- 嚴重度：High
- 重現步驟：連續點「Open again」或快速 Work → Games → Work。
- 預期：LinkedIn 只開一次。
- 實際：LinkedIn、LinkedIn、LinkedIn…
- 根本原因：剛建立、仍在載入的分頁 `url` 為空，只有 `pendingUrl`；去重時被忽略。
- 修復：`tabUrl()` 使用 `url || pendingUrl`；開啟清單本身也先去重。
- 測試：`tests/regression/bug-005-workspace-switching-duplicate-tabs.test.ts`
- 狀態：已修復

### BUG-006 — 共用資源在切換時被關閉
- 嚴重度：High（關錯資源）
- 重現步驟：Work 與 Games 都包含 Discord，Work 設定「Safely Close Apps」；切換 Work → Games。
- 預期：Discord 保持開啟。
- 實際：Discord 收到 WM_CLOSE；啟動 Games 時因行程尚未結束而略過啟動，最後 Games 沒有 Discord。網址同理，會先關閉再重開。
- 修復：離開時排除下一個 Workspace 也需要的 app（相同執行檔或行程名稱）與網址；擴充功能把該分頁的擁有權轉給新的 Workspace。
- 測試：`windows::tests::regression_bug_006_*`、`commands::tests::regression_bug_006_*`
- 狀態：已修復

### BUG-007 — 對瀏覽器執行「安全關閉」會關掉所有分頁
- 嚴重度：Critical
- 重現步驟：把 `chrome.exe` 加入 Work 的 app，Work 設為「Safely Close Apps」，切換到 Games。
- 預期：不影響其他 Workspace 的分頁。
- 實際：所有 Chrome 視窗收到 WM_CLOSE，所有分頁都被關閉。
- 修復：已知瀏覽器（chrome、msedge、firefox、brave、opera、vivaldi…）的「安全關閉」一律改為最小化並顯示說明。另外永遠不會操作 Context Space 自己的行程。
- 測試：`windows::tests::regression_bug_007_browsers_are_never_safe_closed`、`own_process_is_never_targeted`
- 狀態：已修復

### BUG-008 — 新開的分頁不會被存入 Session
- 嚴重度：High（Flow 4 失敗）
- 重現步驟：開啟 Work，另外手動開兩個分頁，切換到 Games，再回到 Work。
- 預期：兩個新分頁會被還原。
- 實際：只有擴充功能自己開的分頁有擁有權，手動開的永遠不會被擷取；此外關閉清單在擷取前就計算好，使用的是舊 Session。
- 修復：新分頁繼承「開啟它的分頁」所屬的 Workspace，否則歸屬目前 active 的 Workspace（桌面 App 無法連線時維持未擁有）；關閉清單改為擷取並儲存後重新讀取。
- 測試：`tests/regression/bug-008-new-tabs-join-active-workspace.test.ts`、`commands::tests::flow_4_*`
- 狀態：已修復

### BUG-009 — 空的擷取結果覆寫 Last Session
- 嚴重度：High（資料遺失）
- 重現步驟：瀏覽器重啟後（擁有權清空）切換 Work → Games。
- 預期：保留 Work 原本的 Last Session。
- 實際：Last Session 被覆寫成 0 個分頁。
- 修復：`save_captured_session` 在沒有任何有效網址時不覆寫，並提示「previous Last Session was kept」；延遲抵達的擷取結果套用同一規則。
- 測試：`db::tests::regression_bug_009_*`、`commands::tests::regression_bug_009_*`
- 狀態：已修復

### BUG-010 — 重啟後依過期狀態關閉 app
- 嚴重度：High
- 重現步驟：Work 為 active 時結束 App（或 App 當機），今天手動開了 VS Code 做別的事，重新開啟 App 後切換到 Games。
- 預期：不根據過期狀態關閉任何東西。
- 實際：VS Code 被最小化或收到 WM_CLOSE，Work 的分頁被關閉。
- 修復：啟動時若有 active Workspace，標記為「recovered」；在使用者重新開啟任一 Workspace 之前，切換都不會關閉分頁或操作 app，並顯示說明（UI 也會提示）。使用者明確按下「Close workspace」時仍照做。
- 測試：`db::tests::regression_bug_010_*`、`commands::tests::regression_bug_010_*`
- 狀態：已修復

### BUG-011 — 刪除後無法排序
- 嚴重度：Medium
- 重現步驟：建立 A、B、C，刪除 B，把 C 往左移。
- 實際：沒有反應（position 為 0 與 2，程式尋找 position 1）。
- 修復：依清單索引交換並重新編號。
- 測試：`db::tests::regression_bug_011_*`
- 狀態：已修復

### BUG-012 — 已刪除的 Workspace 復活
- 嚴重度：Medium
- 重現步驟：開著 Work 的編輯器，從其他地方刪除 Work，再按「Save changes」。
- 實際：`INSERT … ON CONFLICT` 重新建立 Work（資源全部消失）。
- 修復：有 id 但不存在時回傳「Workspace no longer exists.」
- 測試：`db::tests::regression_bug_012_*`、`tests/integration/browser-demo-api.test.ts`
- 狀態：已修復

### BUG-013 — 資料庫損毀時 App 當機
- 嚴重度：Critical
- 重現步驟：把 `context-space.sqlite` 換成損毀檔，或由較新版本建立的資料庫。
- 實際：`setup` 回傳錯誤 → `.expect()` panic，沒有任何視窗或訊息。
- 修復：開啟時執行 `PRAGMA quick_check`；損毀或版本較新時以 Windows 對話框說明檔案位置，並**完全不修改檔案**後結束。遷移改為逐步、每步交易。
- 測試：`db::tests::regression_bug_013_*`、`newer_schema_is_refused_without_changes`、`locked_database_waits_then_reports_instead_of_corrupting`
- 狀態：已修復

### BUG-014 — 編輯器每 5 秒被重置
- 嚴重度：High（輸入遺失、UI 幾乎無法使用）
- 重現步驟：編輯 Work，修改名稱或切到 Browser 分頁，等待 5 秒。
- 實際：名稱還原、分頁跳回 General。
- 根本原因：儀表板每 5 秒刷新一次，產生新的 workspace 物件；編輯器的 `useEffect` 依賴物件本身。
- 修復：只在「對話框開啟」或「換成另一個 Workspace」時重置（依 id）。
- 測試：`tests/regression/bug-014-editor-keeps-unsaved-edits.test.tsx`、E2E `BUG-014: unsaved edits survive…`
- 狀態：已修復

### BUG-015 — Demo API 同步 throw
- 嚴重度：Medium
- 實際：`updateDemo` 在同步函式中 throw，呼叫端的 `.rejects` / `.catch()` 收不到錯誤；integration 測試 2/2 失敗。
- 測試：`tests/regression/bug-015-demo-api-rejects.test.ts`
- 狀態：已修復

### BUG-016 — 切換時 UI 凍結
- 嚴重度：Medium
- 實際：同步的 Tauri command 在主執行緒執行；切換最多等待擴充功能 2.6 秒，且每個 app 呼叫兩次 `System::new_all()`（掃描磁碟、網路等）。視窗與 tray 在這段時間無回應。
- 修復：切換、關閉、還原改為 `#[tauri::command(async)]`；tray 切換在背景執行緒；每次操作只掃描一次行程清單，且只讀取行程資訊。
- 測試：無自動化測試（需要真實 UI 執行緒），列入手動檢查清單。
- 狀態：已修復（手動驗證）

### BUG-017 — 競態條件
- 嚴重度：High
- 實際：(1) 切換鎖是阻塞式 `lock()`，tray 連點會排隊執行多次切換；(2) 刪除不受鎖保護，切換途中刪除會讓 active 指向不存在的 Workspace；(3) 之後每次切換都因「Workspace no longer exists.」而失敗。
- 修復：`try_lock`，忙碌時回傳明確訊息；刪除、關閉、還原共用同一把鎖；找不到前一個 Workspace 時直接略過離開動作。
- 測試：`commands::tests::regression_bug_017_*`、`concurrent_switches_from_many_threads_leave_a_valid_active_workspace`、`rapid_switching_cycle_ends_consistent_without_duplicate_urls`
- 狀態：已修復

### BUG-018 — 啟動參數的 Windows 路徑損壞
- 嚴重度：Medium
- 重現步驟：啟動參數填 `--user-data-dir C:\Users\me\Profile`。
- 實際：`shlex` 把反斜線當跳脫字元，變成 `C:UsersmeProfile`。
- 修復：Windows 規則（空白分隔、雙引號群組、反斜線保留）；工作目錄設為執行檔所在資料夾。
- 測試：`windows::tests::regression_bug_018_*`
- 狀態：已修復

### BUG-019 — URL 正規化不一致
- 嚴重度：Medium
- 實際：擴充功能把整個 URL 轉小寫（`/Jobs` 與 `/jobs` 視為相同）、Rust 只移除根目錄的結尾斜線、資料庫重複檢查使用 `lower()`；三處判斷不同，導致重複或誤判。
- 修復：定義單一規則並寫成共用 fixture `apps/desktop/tests/fixtures/url-normalization.json`，Rust 與兩份 TypeScript 實作都以它測試。規則見 QA_STRATEGY.md。
- 測試：`db::tests::url_normalization_matches_shared_fixture`、`tests/unit/url-normalization.test.ts`
- 狀態：已修復

### BUG-020 — 重新導向的網址重複開啟
- 嚴重度：High
- 重現步驟：Work 釘選 `linkedin.com`（會導向 `https://www.linkedin.com/feed/`），連續開啟 Work 兩次，再切換離開。
- 實際：每次都多開一個 LinkedIn；離開時也不會關閉（URL 已不相符）。
- 修復：擴充功能記錄分頁「當初為哪個網址開啟」（`sourceUrl`），去重與關閉都比對它。
- 測試：`tests/regression/bug-020-redirected-tabs.test.ts`
- 狀態：已修復

### BUG-021 — Tray 與儀表板不同步
- 嚴重度：Medium
- 修復：排序、刪除、儲存、切換後都會重建 tray 選單並發出 `workspace-changed` 事件，儀表板立即刷新；tray 以 ✓ 標示 active。
- 測試：手動檢查清單（tray 無法在 CI 自動化）。
- 狀態：已修復（手動驗證）

### BUG-022 — 可同時執行兩個 App
- 嚴重度：High
- 實際：第二個實例建立第二個 tray icon；bridge port 已被占用，只寫一行 log，擴充功能連到的是另一個實例。
- 修復：`tauri-plugin-single-instance`，第二次啟動只會把現有視窗帶到前景。
- 測試：手動檢查清單。
- 狀態：已修復（手動驗證）

### BUG-023 — Popup 的錯誤訊息誤導
- 嚴重度：Medium
- 實際：桌面 App 沒開時，popup 顯示 token 設定畫面與「TypeError: Failed to fetch」，看起來像 token 錯誤；加入重複分頁時仍顯示「Added 1 tab.」
- 修復：區分「桌面 App 未執行」、「token 被拒絕」與其他錯誤；顯示新增與重複的數量。
- 測試：`tests/regression/bug-023-popup-offline-message.test.ts`
- 狀態：已修復

### BUG-024 — 失敗時對話框仍關閉
- 嚴重度：Low
- 修復：只有成功時才關閉對話框或清空輸入。
- 測試：E2E `an empty name is refused and the dialog stays open…`、`a duplicate URL is refused…`（驗證輸入被保留）
- 狀態：已修復

### BUG-025 — Toast 被提早關閉
- 嚴重度：Low
- 測試：`tests/regression/bug-025-033-feedback-messages.test.tsx`
- 狀態：已修復

### BUG-026 — QA gate 本身壞掉
- 嚴重度：Medium
- 實際：`cargo fmt --check` 失敗、clippy 警告、`npm test`（apps/desktop）會用 Vitest 執行 Playwright spec、E2E 的 `addInitScript(localStorage.clear)` 在 reload 時也清空資料（使「重啟後仍存在」的測試永遠失敗）、`getByRole('heading', { name: 'Work' })` 同時比對到「My workspaces」。
- 修復：格式化、修正 clippy、Vitest 排除 `tests/e2e`、E2E 只在第一次載入時清空、使用精確 selector、改以 production build（`vite preview`）執行 E2E。lint 改為 warnings 視為錯誤，並納入擴充功能與 clippy。
- 狀態：已修復

### BUG-027 — 擴充功能離線時無法去重
- 嚴重度：Medium
- 實際：沒有擴充功能時，每次切換都用預設瀏覽器重新開啟所有釘選網址。
- 目前：會顯示「Browser extension is not connected. N page(s) opened in your default browser, so tabs that were already open could not be reused.」
- 未修復原因：沒有擴充功能就無法得知已開啟的分頁。可能的改善：擴充功能離線時改為詢問是否開啟。
- 測試：`commands::tests::extension_offline_degrades_with_clear_warnings_and_no_commands`
- 狀態：部分修復

### BUG-028 — Chrome 與 Edge 共用指令佇列
- 嚴重度：Medium
- 重現步驟：Chrome 與 Edge 都安裝並設定擴充功能，切換 Workspace。
- 實際：兩個瀏覽器輪詢同一個佇列，擷取、關閉、開啟指令會隨機交給其中一個，Session 只包含一個瀏覽器的分頁。
- 建議修復：每個擴充功能實例註冊自己的 client id，桌面端依 client 分送指令並合併擷取結果。
- 狀態：開放（目前建議只在一個瀏覽器安裝擴充功能，已寫入 README）

### BUG-029 — 空 token 被接受
- 嚴重度：Low
- 修復：token 為空或長度不符一律拒絕，並以固定時間比較。
- 測試：`bridge::tests::regression_bug_029_*`
- 狀態：已修復

### BUG-030 — 多行程 app 產生大量警告
- 嚴重度：Low
- 修復：同一個 app 的所有行程合併處理，只有完全找不到可見視窗時才警告一次。
- 測試：`windows::tests::keep_running_does_nothing_and_minimize_targets_all_app_processes`
- 狀態：已修復

### BUG-031 — 擁有權寫入遺失
- 嚴重度：Medium
- 實際：多個分頁同時建立時，read-modify-write 互相覆蓋，部分分頁失去擁有權（不會被擷取也不會被關閉）。
- 修復：所有擁有權更新依序執行。
- 測試：`tests/regression/bug-031-concurrent-owner-writes.test.ts`
- 狀態：已修復

### BUG-032 — `localhost:3000` 被拒絕
- 嚴重度：Low
- 測試：`tests/regression/bug-032-host-with-port-input.test.ts`
- 狀態：已修復

### BUG-033 — 編輯器錯誤對螢幕閱讀器不可見
- 嚴重度：Medium（無障礙）
- 實際：對話框開啟時 Radix 會把頁面其他部分設為 `aria-hidden`，而錯誤只顯示在 toast。
- 修復：錯誤也顯示在對話框內的 `role="alert"` 區域。
- 測試：`tests/regression/bug-025-033-feedback-messages.test.tsx`、E2E 驗證 dialog 內的 alert
- 狀態：已修復

## 4. 本次同時加入的除錯能力

- 結構化事件記錄（本機、記憶體中最近 200 筆，debug build 另寫入 log）：`WORKSPACE_OPEN/CLOSE`、`WORKSPACE_SWITCH_START/COMPLETE`、`SESSION_SAVE/RESTORE`、`TAB_OPEN/CLOSE`、`APP_LAUNCH/MINIMIZE/SAFE_CLOSE`、`EXTENSION_CONNECTED/DISCONNECTED`、`BRIDGE_COMMAND_EXPIRED`、`WORKSPACE_RECOVERED` 等。只記錄 id、名稱與數量，不記錄網址或分頁標題，也不送出本機。
- Debug 面板：`Ctrl+Shift+D` 或網址加上 `?debug`。顯示 active Workspace（含 recovered 狀態）、擴充功能連線與最後連線時間、資料庫路徑/狀態/schema 版本、Last Session 分頁數、追蹤中的 app 是否執行、最後一次轉換、最近事件。

## 5. 無法自動驗證、需人工確認的項目

- 真實 Win32 最小化 / WM_CLOSE 對 VS Code、Discord、Steam、Spotify、Terminal 的效果
- Tray（單一 icon、選單內容、從 tray 切換）
- 真實 Chrome / Edge 載入擴充功能、權限、瀏覽器重啟
- UI 在切換期間不凍結（BUG-016）

以上都列在 [MANUAL_TEST_CHECKLIST.md](MANUAL_TEST_CHECKLIST.md)。
