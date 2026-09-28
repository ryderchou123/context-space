# Context Space 未來路線圖

## V1.1 — 更快、更可靠的日常切換

- 可設定全域鍵盤快捷鍵，預設候選為 `Ctrl + Alt + Space`。
- 改善 browser session recovery 與 tab ownership，處理 browser crash、extension suspend 與多個 browser windows。
- Workspace 搜尋與 keyboard-first switcher。
- 改善 tray menu：recent Workspace、extension 狀態、可設定 shortcut。
- 匯入／匯出 Workspace JSON，含 schema version 與衝突預覽。
- Bridge port fallback、token rotation 與 extension reconnect diagnostics。

## V1.5 — Workspace Snapshots

Snapshot 是具名、不可被 Last Session 自動覆寫的一組 tabs 與 apps：

```text
Work
├── Default
├── Last Session
└── Snapshots
    ├── NVIDIA Application
    ├── Amazon Interview
    └── Resume Update
```

支援建立、命名、覆寫、複製、刪除與選擇性還原；Snapshot 應保存 URL、app identity 與未來可選的 window layout。

## V2 — Personal Context Memory System

### Focus Mode

- 啟動 Work Focus Mode 時隱藏或關閉 Games tabs、最小化 Discord、打開 Work resources。
- 可選 distraction website blocking，預設關閉並提供明確 bypass。

### Window Position Restoration

保存 monitor identity、window position、size、maximized/minimized state。例如 Monitor 1 的 Chrome 左半、Monitor 2 的 VS Code 全螢幕。

### Multi-monitor Workspace Restore

處理顯示器拔除、DPI 變更、解析度改變與 display identity fallback，安全地還原完整 Workspace layout。

### Better Browser Integration

- Firefox、Brave 與技術上可行時的 Arc。
- Browser profiles、tab groups、window ownership 與跨 browser session。

### Automation

- 9 AM 開啟 Work；6 PM 保存 Work 並切換 Personal。
- Trigger 支援時間、登入、顯示器／網路狀態；每個 automation 可暫停並有執行紀錄。

### Workspace Templates

- Developer：GitHub、VS Code、Terminal、Docs。
- Student：Canvas、Piazza、Notes。
- Gaming：Steam、Discord、Twitch。
- Job Search：LinkedIn、Handshake、Resume、Job boards。

### Cloud Sync

維持 local-first；同步是可選功能。設計 end-to-end encryption、device merge、離線衝突與完整 data export。

### Optional AI Features

AI 不會成為核心功能或預設上傳 browsing activity。未來可在明確 opt-in 下：建議 tab 所屬 Workspace、摘要 session、找出 stale tabs、建議關閉、偵測未完成工作，或提示「你似乎正在處理 NVIDIA 申請，是否將這 6 個 tabs 保存為 Snapshot？」優先評估 on-device model 與最小資料揭露。
