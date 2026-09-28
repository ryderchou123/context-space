# Context Space — MVP V1 產品與工程計畫

## 產品目標

降低使用者在工作、課業、娛樂與個人活動之間切換時的重建成本。V1 必須能真正保存瀏覽器情境、啟動資源、安全處理上一個 Workspace，並在重啟應用程式後保留設定。

## 使用者問題

- 每次切換活動都要重新尋找分頁與啟動程式。
- 一次關閉太多程式可能造成未保存內容遺失。
- Browser bookmark 無法描述桌面應用程式、session 與離開行為。
- 全域規則不適合所有活動；Work 與 Games 需要不同策略。

## 產品原則

**Fast、Simple、Local、Safe、Predictable。** 預設 minimize，不 force kill；使用者能看見 active Workspace 與 extension 狀態；瀏覽資料不離開本機。

## User stories

- 身為使用者，我可以建立 Work、Homework、Games 並調整順序。
- 我可以把 pinned websites、saved websites 與 desktop apps 加入個別 Workspace。
- 我可以為每個 Workspace 設定離開時保留／關閉 tabs，以及保留／最小化／安全關閉 apps。
- 我可以切換 Workspace，保存前一個 session 並還原下一個 session。
- 我可以從 Chrome／Edge popup 選取目前開啟的 tabs 加入 Workspace。
- 我可以關閉 Dashboard 並從 system tray 快速切換或再次開啟。

## MVP 範圍

Windows desktop app、SQLite persistence、Workspace CRUD/reorder、URL/app resources、pinned resources、Last Session、per-Workspace behavior、process duplicate prevention、Win32 minimize/graceful close、tray、Chrome/Edge MV3 extension、token-protected localhost bridge、dark/light theme、onboarding、unit/component tests 與 NSIS build。

## Non-goals

macOS/Linux、Firefox、force kill、browser history、account、cloud sync、telemetry、AI、scheduling、named snapshots、window geometry、多螢幕 layout、正式 extension store publishing 與 code signing。

## Architecture

Tauri 2 提供小型 native shell 與 Rust trust boundary；React/TypeScript renderer 專注互動；SQLite 由 Rust 管理；Win32 adapter 處理 window；MV3 extension 經 loopback HTTP bridge 溝通。詳細圖與 schema 見 `docs/architecture.md`。

## Browser integration design

採 localhost bridge 而非 Native Messaging。原因是 MVP 安裝與開發流程較單純，不需為 Chrome/Edge 分別寫入 native host registry。安全措施包含 loopback-only listener、隨機 token、最小 extension permissions、HTTP/HTTPS URL validation 與無外部 server。代價是 Desktop 必須先啟動，且 extension 以 polling 接收命令。

## Windows integration design

Rust 使用 process executable/name 判斷是否已執行，以 `std::process::Command` 傳入分離 arguments。Window action 使用 PID + top-level HWND；最小化走 `SW_MINIMIZE`，關閉走 `WM_CLOSE`。任何 partial failure 轉成 UI warning，絕不 force terminate。

## Storage model

SQLite 包含 Workspace、browser/app resources、workspace session、session tabs 與 app state。外鍵與 cascade delete 開啟；資料庫存於 Tauri app data directory。未採 ORM，以小型、明確的 parameterized SQL 保持 schema 可見性。

## 重要工程決策

1. 採 Tauri 而非 Electron；環境補齊 Rust MSVC/C++ toolchain 後可滿足原始偏好。
2. 採 bundled SQLite，降低使用者另外安裝 runtime 的需求。
3. 採 NSIS 而非 MSI，避免 VBSCRIPT optional feature 相依。
4. Extension service worker 不依賴記憶體存活，token 放在 `chrome.storage.local`，polling 由 alarms 驅動。
5. V1 不實作 force close；safe close 對未保存內容保留應用程式自己的 prompt。
6. 全域 `Ctrl+Alt+Space` 延後至 V1.1，避免和既有系統／應用程式 shortcut 衝突，並提供使用者可設定介面。

## 風險

- Chromium worker 有 suspend lifecycle，因此 command 可能最多延遲一個 alarm interval。
- 某些 multi-process app 的 executable 與 visible window 不在同一 PID；V1 會回報 warning。
- 未簽署 installer 可能觸發 Windows SmartScreen。
- V1 session capture 是 browser-level capture；更精準的 tab ownership 將在 V1.1 改善。
- Port 47651 被占用時 extension bridge 無法啟動；Desktop 仍可使用其餘功能。

## 測試策略

- Rust unit tests：Workspace CRUD、SQLite cascade、URL normalize/duplicate、session save/restore、switch URL plan。
- Vitest：URL normalization、tab dedupe、session merge、settings override、主要 Workspace card semantics。
- Build validation：TypeScript strict build、oxlint、extension typecheck/build、cargo fmt/test、Tauri NSIS production build。
- 手動 smoke：啟動 desktop、建立 Workspace、確認 tray、bridge health、load-unpacked popup。

## Release criteria

所有自動測試與 lint 通過；React、extension、Rust 與 NSIS build 成功；應用程式可啟動且不 crash；資料重啟後存在；README 的安裝流程可重現；git clean 且 main 已推送；無聲稱未驗證的功能。
