# 發佈前手動測試清單

自動化測試無法涵蓋真實瀏覽器、真實 Windows app 與 tray。每次發佈前依序完成本清單。

**準備**

- [ ] `npm run qa:full` 全部通過
- [ ] 安裝 `npm run build` 產生的安裝檔（`apps/desktop/src-tauri/target/release/bundle/nsis/`）
- [ ] Chrome 或 Edge **擇一**載入 `apps/extension/dist`（兩個都裝會造成 BUG-028）
- [ ] 把儀表板上的 bridge token 貼到擴充功能 popup，popup 顯示「Desktop connected」
- [ ] 開啟 Debug 面板（`Ctrl+Shift+D`），確認「Extension: Connected」與「Database: ok」
- [ ] 準備測試 app：VS Code、Discord、Steam、Spotify、Windows Terminal（至少三個）

建議建立三個 Workspace：
- **Work**：`linkedin.com`、`github.com`；VS Code；瀏覽器「Close workspace tabs」、app「Minimize」
- **Games**：`twitch.tv`；Discord、Steam；app「Safely close」
- **Homework**：`docs.google.com`；Spotify；全部「Keep」

## Workspace

- [ ] 建立 Workspace（名稱、圖示、顏色）
- [ ] 編輯 Workspace：改名稱後等 10 秒再儲存，輸入沒有消失（BUG-014）
- [ ] 在 Browser 分頁停留 10 秒，不會跳回 General
- [ ] 空白名稱無法儲存，對話框內顯示錯誤
- [ ] 左右移動排序；刪除一個後再排序仍正常（BUG-011）
- [ ] 刪除 Workspace（確認對話框可取消）
- [ ] 完全結束 App（tray → Quit）再開啟，Workspace、網址、app、設定都還在
- [ ] 很長的名稱不會超出卡片；30 個以上 Workspace 可以捲動

## Browser

- [ ] 開啟 Work：LinkedIn、GitHub 在背景分頁開啟
- [ ] 再按一次「Open again」：**沒有**重複分頁（BUG-005）
- [ ] LinkedIn 導向 `www.linkedin.com/feed/` 後再開一次 Work：仍然只有一個（BUG-020）
- [ ] popup「Add current tab」：加入成功；再加一次顯示「already in this workspace」
- [ ] popup「Choose open tabs」選多個加入
- [ ] Work 期間手動開兩個新分頁 → 切到 Games → 回到 Work：兩個分頁被還原
- [ ] 儲存 current tabs：切換後 Debug 面板「Last Session tabs」數量正確
- [ ] 關閉 Workspace tabs：只有 Work 的分頁被關閉
- [ ] **自己事先開的 `linkedin.com` 分頁不會被關閉**（BUG-001）
- [ ] **Games 的 twitch.tv 不會因 Work 被關閉**
- [ ] `linkedin.com/jobs`、`linkedin.com/messages`（自己開的）不會因為 Work 有 `linkedin.com` 而被關閉
- [ ] Restore Last Session：還原的分頁正確，已開啟的不重複
- [ ] 關閉瀏覽器再開啟（還原分頁）→ 切換 Workspace：還原回來的分頁**不會**被關閉（BUG-003），Last Session 沒被清空（BUG-009）
- [ ] 桌面 App 關閉時開 popup：顯示「Context Space is not running」，不是 token 畫面（BUG-023）
- [ ] 輸入錯誤 token：顯示「token was rejected」

## Applications

- [ ] App 可以啟動（含啟動參數，例如路徑 `C:\Users\...`，BUG-018）
- [ ] 已開啟的 app 不會一直重開
- [ ] Minimize：視窗最小化，工作管理員中行程仍在
- [ ] Keep Running：完全沒有變化
- [ ] Safe Close：app 自己關閉；有未儲存內容時 app 會詢問（例如 VS Code 未儲存檔案）
- [ ] Safe Close **不會** force kill：取消儲存詢問後 app 仍在執行
- [ ] 把 `chrome.exe` / `msedge.exe` 加入 app 並設 Safe Close：切換後瀏覽器只被最小化，分頁都在（BUG-007）
- [ ] Work 與 Games 都有 Discord：Work → Games 時 Discord 不被關閉或最小化（BUG-006）
- [ ] 執行檔被移動或刪除：切換時顯示「missing or was moved」，其他資源正常
- [ ] VS Code（多行程）最小化時只有必要的警告（BUG-030）

## Switching

- [ ] Work → Games：Work session 已儲存、Work 分頁關閉、Work app 最小化、Games 資源啟動、Games 變 active
- [ ] Games → Homework
- [ ] Homework → Work
- [ ] 同一個 Workspace 再開一次：不重複、不關閉任何東西
- [ ] 快速連點 / 雙擊開啟按鈕：只執行一次，沒有重複分頁或 app
- [ ] tray 快速連續切換三次：只有一個 active，其他顯示「still running」而被略過（BUG-017）
- [ ] 切換期間視窗可以拖動、tray 可以開啟（沒有凍結，BUG-016）
- [ ] 擴充功能停用時切換：顯示「Browser extension is not connected」，App 不當機

## Restart recovery

- [ ] Work 為 active 時從工作管理員結束 Context Space → 重新開啟：
  - [ ] 儀表板顯示「Restored after Context Space restarted…」
  - [ ] 切換到 Games 時 **不會**關閉 Work 分頁或 app，並顯示說明（BUG-010）
  - [ ] 再開啟任一 Workspace 後，一般行為恢復
- [ ] 重新開機後同上

## System tray

- [ ] 只有一個 tray icon
- [ ] 再次執行安裝好的 App：不會出現第二個 icon，現有視窗被帶到前景（BUG-022）
- [ ] 關閉視窗 → 縮小到 tray；左鍵點 icon 或「Open Dashboard」可還原
- [ ] 選單顯示「Active: Work」，active 項目有 ✓
- [ ] 從 tray 切換 Workspace，儀表板立即更新（BUG-021）
- [ ] 新增 / 改名 / 刪除 / 排序後 tray 選單立即更新
- [ ] Quit 會真正結束（工作管理員中沒有殘留）

## Persistence

- [ ] 重啟 App 後資料仍存在
- [ ] Last Session 存在
- [ ] Settings（每個 Workspace 的切換行為）存在
- [ ] 刪除的 Workspace 重啟後沒有回來
- [ ] 資料庫損毀測試（用複本）：把 `%APPDATA%\com.contextspace.desktop\context-space.sqlite` 換成文字檔 → 啟動時顯示錯誤對話框，檔案內容**未被修改**（BUG-013）

## UI

- [ ] 最小視窗（900×640）沒有重疊、裁切或水平捲動
- [ ] 最大化視窗版面正常
- [ ] 深色 / 淺色主題切換後文字清楚可讀
- [ ] 只用鍵盤可以建立 Workspace（Tab、Enter、Esc）
- [ ] 沒有網址或 app 的空 Workspace 可以開啟
- [ ] 大量資源（20 個網址、5 個 app）的編輯器可以捲動

## 發佈

- [ ] `docs/QA_REPORT.md` 已更新本次結果
- [ ] Critical = 0，未解決的 High regression = 0

## 2026-09-29 執行狀態

- [x] 測試前建立 live SQLite 備份，測試後以 SHA-256 驗證還原完成。
- [x] 真實 Tauri WebView 的 Workspace CRUD、輸入驗證、資源新增、主題 persistence、Debug database status 與 BUG-014 編輯器穩定性。
- [x] 真實 Edge unpacked MV3 載入、bridge 驗證與 popup `Desktop connected`。
- [x] 一次性 Win32 測試視窗的 minimize、程序存活與 `WM_CLOSE`。
- [ ] 真實 Edge 完整 Workspace open/capture/close/restore/restart（互動式桌面目前不可用）。
- [ ] Tray menu、tray switch、single-instance focus 與原生 resize（原生 UI automation 目前不可用）。
- [ ] 真實 Tauri restart recovery（需 VM、獨立 Windows 使用者，或可真正隔離 Known Folder 的測試環境）。
