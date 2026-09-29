export type BrowserExitBehavior = 'keep' | 'close'
export type AppExitBehavior = 'keep' | 'minimize' | 'safe_close'

export interface BrowserResource {
  id: string
  workspaceId: string
  title: string
  url: string
  pinned: boolean
  enabled: boolean
  position: number
}

export interface AppResource {
  id: string
  workspaceId: string
  displayName: string
  executablePath: string
  processName: string
  launchArgs: string
  enabled: boolean
  exitBehaviorOverride?: AppExitBehavior
}

export interface SessionTab { title: string; url: string }

export interface Workspace {
  id: string
  name: string
  icon: string
  color: string
  position: number
  browserExitBehavior: BrowserExitBehavior
  appExitBehavior: AppExitBehavior
  saveSessionOnExit: boolean
  restoreSessionOnLaunch: boolean
  browserResources: BrowserResource[]
  appResources: AppResource[]
  lastSession: SessionTab[]
  createdAt: string
  updatedAt: string
}

export interface AppSnapshot {
  workspaces: Workspace[]
  activeWorkspaceId?: string
  activeWorkspaceRecovered?: boolean
  extensionConnected: boolean
  bridgeToken: string
  bridgePort: number
}

export interface WorkspaceDraft {
  id?: string
  name: string
  icon: string
  color: string
  browserExitBehavior: BrowserExitBehavior
  appExitBehavior: AppExitBehavior
  saveSessionOnExit: boolean
  restoreSessionOnLaunch: boolean
}

export interface OperationResult { message: string; warnings: string[] }

export interface LogEntry { at: string; event: string; detail: string }

export interface DebugInfo {
  activeWorkspaceId?: string
  activeWorkspaceName?: string
  activeWorkspaceRecovered: boolean
  extensionConnected: boolean
  extensionLastSeenSecs?: number
  databasePath: string
  databaseStatus: string
  schemaVersion: number
  lastSessionTabs: number
  trackedApps: { displayName: string; running: boolean }[]
  lastTransition?: LogEntry
  recentEvents: LogEntry[]
}
