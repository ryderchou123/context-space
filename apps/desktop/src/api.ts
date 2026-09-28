import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { AppSnapshot, OperationResult, WorkspaceDraft } from './types'

const isTauri = '__TAURI_INTERNALS__' in window
const demoSnapshot: AppSnapshot = { workspaces: [], extensionConnected: false, bridgeToken: 'Run the desktop app to generate a bridge token', bridgePort: 47651 }

export const desktopApi = {
  snapshot: async (): Promise<AppSnapshot> => isTauri ? invoke('get_snapshot') : demoSnapshot,
  saveWorkspace: (draft: WorkspaceDraft): Promise<AppSnapshot> => invoke('save_workspace', { draft }),
  deleteWorkspace: (id: string): Promise<AppSnapshot> => invoke('delete_workspace', { id }),
  reorderWorkspace: (id: string, direction: number): Promise<AppSnapshot> => invoke('reorder_workspace', { id, direction }),
  addBrowserResource: (workspaceId: string, title: string, url: string, pinned: boolean): Promise<AppSnapshot> => invoke('add_browser_resource', { workspaceId, title, url, pinned }),
  removeBrowserResource: (id: string): Promise<AppSnapshot> => invoke('remove_browser_resource', { id }),
  toggleBrowserResource: (id: string, enabled: boolean, pinned: boolean): Promise<AppSnapshot> => invoke('toggle_browser_resource', { id, enabled, pinned }),
  async chooseExecutable(): Promise<string | null> {
    if (!isTauri) return null
    const result = await open({ multiple: false, filters: [{ name: 'Applications', extensions: ['exe', 'bat', 'cmd'] }] })
    return typeof result === 'string' ? result : null
  },
  addAppResource: (workspaceId: string, displayName: string, executablePath: string, processName: string, launchArgs: string): Promise<AppSnapshot> => invoke('add_app_resource', { workspaceId, displayName, executablePath, processName, launchArgs }),
  removeAppResource: (id: string): Promise<AppSnapshot> => invoke('remove_app_resource', { id }),
  switchWorkspace: (id: string, restoreSession?: boolean): Promise<OperationResult> => invoke('switch_workspace', { id, restoreSession }),
  closeWorkspace: (id: string): Promise<OperationResult> => invoke('close_workspace', { id }),
  restoreSession: (id: string): Promise<OperationResult> => invoke('restore_session', { id }),
}
