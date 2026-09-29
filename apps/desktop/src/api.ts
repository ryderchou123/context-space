import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { normalizeUrl } from './core'
import type { AppSnapshot, DebugInfo, OperationResult, Workspace, WorkspaceDraft } from './types'

const isTauri = '__TAURI_INTERNALS__' in window
const DEMO_KEY = 'context-space-browser-demo'
const MISSING = 'Workspace no longer exists.'

function emptySnapshot(): AppSnapshot {
  return { workspaces: [], extensionConnected: false, bridgeToken: 'Desktop bridge is available in the installed app', bridgePort: 47651 }
}

function readDemo(): AppSnapshot {
  try {
    const parsed = JSON.parse(localStorage.getItem(DEMO_KEY) ?? '') as AppSnapshot
    return parsed && Array.isArray(parsed.workspaces) ? parsed : emptySnapshot()
  } catch {
    return emptySnapshot()
  }
}

function writeDemo(snapshot: AppSnapshot): AppSnapshot {
  localStorage.setItem(DEMO_KEY, JSON.stringify(snapshot))
  return structuredClone(snapshot)
}

// Async so validation errors become rejected promises, exactly like the Tauri commands.
async function updateDemo(update: (snapshot: AppSnapshot) => void): Promise<AppSnapshot> {
  const snapshot = readDemo()
  update(snapshot)
  return writeDemo(snapshot)
}

function workspaceOrThrow(snapshot: AppSnapshot, id: string): Workspace {
  const workspace = snapshot.workspaces.find((item) => item.id === id)
  if (!workspace) throw new Error(MISSING)
  return workspace
}

export const desktopApi = {
  isDesktop: isTauri,
  snapshot: async (): Promise<AppSnapshot> => isTauri ? invoke('get_snapshot') : readDemo(),
  /** Subscribes to changes made outside this window (tray, extension). Returns an unsubscribe. */
  async onChanged(listener: () => void): Promise<() => void> {
    if (!isTauri) return () => {}
    return listen('workspace-changed', listener)
  },
  saveWorkspace: (draft: WorkspaceDraft): Promise<AppSnapshot> => isTauri
    ? invoke('save_workspace', { draft })
    : updateDemo((snapshot) => {
      if (!draft.name.trim()) throw new Error('Workspace name is required.')
      const existing = draft.id ? workspaceOrThrow(snapshot, draft.id) : undefined
      const now = new Date().toISOString()
      const next: Workspace = {
        ...draft,
        name: draft.name.trim(),
        id: existing?.id ?? crypto.randomUUID(),
        position: existing?.position ?? snapshot.workspaces.length,
        browserResources: existing?.browserResources ?? [],
        appResources: existing?.appResources ?? [],
        lastSession: existing?.lastSession ?? [],
        createdAt: existing?.createdAt ?? now,
        updatedAt: now,
      }
      if (existing) snapshot.workspaces[snapshot.workspaces.indexOf(existing)] = next
      else snapshot.workspaces.push(next)
    }),
  deleteWorkspace: (id: string): Promise<AppSnapshot> => isTauri
    ? invoke('delete_workspace', { id })
    : updateDemo((snapshot) => {
      snapshot.workspaces = snapshot.workspaces.filter((item) => item.id !== id)
      if (snapshot.activeWorkspaceId === id) delete snapshot.activeWorkspaceId
    }),
  reorderWorkspace: (id: string, direction: number): Promise<AppSnapshot> => isTauri
    ? invoke('reorder_workspace', { id, direction })
    : updateDemo((snapshot) => {
      const index = snapshot.workspaces.findIndex((item) => item.id === id)
      const target = index + direction
      if (index < 0 || target < 0 || target >= snapshot.workspaces.length) return
      ;[snapshot.workspaces[index], snapshot.workspaces[target]] = [snapshot.workspaces[target], snapshot.workspaces[index]]
      snapshot.workspaces.forEach((item, position) => { item.position = position })
    }),
  addBrowserResource: (workspaceId: string, title: string, url: string, pinned: boolean): Promise<AppSnapshot> => isTauri
    ? invoke('add_browser_resource', { workspaceId, title, url, pinned })
    : updateDemo((snapshot) => {
      const workspace = workspaceOrThrow(snapshot, workspaceId)
      const normalized = normalizeUrl(url)
      if (workspace.browserResources.some((item) => normalizeUrl(item.url) === normalized)) throw new Error('That URL is already in this workspace.')
      workspace.browserResources.push({ id: crypto.randomUUID(), workspaceId, title: title.trim() || normalized, url: normalized, pinned, enabled: true, position: workspace.browserResources.length })
    }),
  removeBrowserResource: (id: string): Promise<AppSnapshot> => isTauri
    ? invoke('remove_browser_resource', { id })
    : updateDemo((snapshot) => { snapshot.workspaces.forEach((workspace) => { workspace.browserResources = workspace.browserResources.filter((item) => item.id !== id) }) }),
  toggleBrowserResource: (id: string, enabled: boolean, pinned: boolean): Promise<AppSnapshot> => isTauri
    ? invoke('toggle_browser_resource', { id, enabled, pinned })
    : updateDemo((snapshot) => { snapshot.workspaces.flatMap((workspace) => workspace.browserResources).filter((item) => item.id === id).forEach((item) => { item.enabled = enabled; item.pinned = pinned }) }),
  async chooseExecutable(): Promise<string | null> {
    if (!isTauri) return null
    const result = await open({ multiple: false, filters: [{ name: 'Applications', extensions: ['exe', 'bat', 'cmd'] }] })
    return typeof result === 'string' ? result : null
  },
  addAppResource: (workspaceId: string, displayName: string, executablePath: string, processName: string, launchArgs: string): Promise<AppSnapshot> => isTauri
    ? invoke('add_app_resource', { workspaceId, displayName, executablePath, processName, launchArgs })
    : updateDemo((snapshot) => { const workspace = workspaceOrThrow(snapshot, workspaceId); workspace.appResources.push({ id: crypto.randomUUID(), workspaceId, displayName, executablePath, processName, launchArgs, enabled: true }) }),
  removeAppResource: (id: string): Promise<AppSnapshot> => isTauri
    ? invoke('remove_app_resource', { id })
    : updateDemo((snapshot) => { snapshot.workspaces.forEach((workspace) => { workspace.appResources = workspace.appResources.filter((item) => item.id !== id) }) }),
  switchWorkspace: (id: string, restoreSession?: boolean): Promise<OperationResult> => isTauri
    ? invoke('switch_workspace', { id, restoreSession })
    : updateDemo((snapshot) => { workspaceOrThrow(snapshot, id); snapshot.activeWorkspaceId = id; snapshot.activeWorkspaceRecovered = false }).then((snapshot) => ({ message: `${workspaceOrThrow(snapshot, id).name} is now active.`, warnings: [] })),
  closeWorkspace: (id: string): Promise<OperationResult> => isTauri
    ? invoke('close_workspace', { id })
    : updateDemo((snapshot) => { workspaceOrThrow(snapshot, id); if (snapshot.activeWorkspaceId === id) delete snapshot.activeWorkspaceId }).then(() => ({ message: 'Workspace was closed safely.', warnings: [] })),
  restoreSession: (id: string): Promise<OperationResult> => isTauri
    ? invoke('restore_session', { id })
    : updateDemo((snapshot) => { workspaceOrThrow(snapshot, id) }).then((snapshot) => ({ message: `Restored ${workspaceOrThrow(snapshot, id).lastSession.length} saved tabs.`, warnings: [] })),
  debugInfo: async (): Promise<DebugInfo> => {
    if (isTauri) return invoke('get_debug_info')
    const snapshot = readDemo()
    const active = snapshot.workspaces.find((item) => item.id === snapshot.activeWorkspaceId)
    return { activeWorkspaceId: active?.id, activeWorkspaceName: active?.name, activeWorkspaceRecovered: false, extensionConnected: false, databasePath: `browser localStorage (${DEMO_KEY})`, databaseStatus: `ok (${snapshot.workspaces.length} workspaces)`, schemaVersion: 0, lastSessionTabs: active?.lastSession.length ?? 0, trackedApps: active?.appResources.map((app) => ({ displayName: app.displayName, running: false })) ?? [], recentEvents: [] }
  },
}
