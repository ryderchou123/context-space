import { beforeEach, describe, expect, it } from 'vitest'
import { desktopApi } from '../../src/api'
import type { WorkspaceDraft } from '../../src/types'

// Exercises the browser demo backend. The SQLite backend has the same cases in
// src-tauri/src/db.rs (run with `npm run test:rust`).
const draft = (name: string, extra: Partial<WorkspaceDraft> = {}): WorkspaceDraft => ({ name, icon: 'briefcase', color: '#0D9488', browserExitBehavior: 'close', appExitBehavior: 'minimize', saveSessionOnExit: true, restoreSessionOnLaunch: true, ...extra })
const names = async () => (await desktopApi.snapshot()).workspaces.map((w) => w.name)

describe('workspace', () => {
  beforeEach(() => localStorage.clear())

  it('creates a workspace with a trimmed name', async () => {
    const snapshot = await desktopApi.saveWorkspace(draft('  Work  '))
    expect(snapshot.workspaces.map((w) => w.name)).toEqual(['Work'])
  })

  it('rejects an empty name', async () => {
    await expect(desktopApi.saveWorkspace(draft('   '))).rejects.toThrow('name is required')
  })

  it('renames and persists settings without losing resources', async () => {
    const id = (await desktopApi.saveWorkspace(draft('Work'))).workspaces[0].id
    await desktopApi.addBrowserResource(id, 'LinkedIn', 'linkedin.com', true)
    await desktopApi.saveWorkspace(draft('Deep Work', { id, browserExitBehavior: 'keep', appExitBehavior: 'safe_close', saveSessionOnExit: false }))
    const [work] = (await desktopApi.snapshot()).workspaces
    expect(work).toMatchObject({ name: 'Deep Work', browserExitBehavior: 'keep', appExitBehavior: 'safe_close', saveSessionOnExit: false })
    expect(work.browserResources).toHaveLength(1)
  })

  it('deletes a workspace and clears it as active', async () => {
    const id = (await desktopApi.saveWorkspace(draft('Temp'))).workspaces[0].id
    await desktopApi.switchWorkspace(id)
    const snapshot = await desktopApi.deleteWorkspace(id)
    expect(snapshot.workspaces).toEqual([])
    expect(snapshot.activeWorkspaceId).toBeUndefined()
  })

  it('reorders and ignores moves past either edge', async () => {
    await desktopApi.saveWorkspace(draft('Work'))
    await desktopApi.saveWorkspace(draft('Games'))
    const games = (await desktopApi.snapshot()).workspaces[1].id
    await desktopApi.reorderWorkspace(games, -1)
    expect(await names()).toEqual(['Games', 'Work'])
    await desktopApi.reorderWorkspace(games, -1)
    expect(await names()).toEqual(['Games', 'Work'])
  })
})

describe('browser resources', () => {
  beforeEach(() => localStorage.clear())

  it('adds, detects duplicates after normalization, and removes', async () => {
    const id = (await desktopApi.saveWorkspace(draft('Work'))).workspaces[0].id
    let snapshot = await desktopApi.addBrowserResource(id, '', 'https://linkedin.com/#top', true)
    expect(snapshot.workspaces[0].browserResources[0]).toMatchObject({ url: 'https://linkedin.com', title: 'https://linkedin.com' })
    await expect(desktopApi.addBrowserResource(id, 'Again', 'linkedin.com/', true)).rejects.toThrow('already in this workspace')
    snapshot = await desktopApi.addBrowserResource(id, 'Jobs', 'linkedin.com/jobs', true)
    expect(snapshot.workspaces[0].browserResources).toHaveLength(2)
    snapshot = await desktopApi.removeBrowserResource(snapshot.workspaces[0].browserResources[0].id)
    expect(snapshot.workspaces[0].browserResources.map((r) => r.title)).toEqual(['Jobs'])
  })

  it('rejects malformed and non-web URLs', async () => {
    const id = (await desktopApi.saveWorkspace(draft('Work'))).workspaces[0].id
    await expect(desktopApi.addBrowserResource(id, 'Bad', 'javascript:alert(1)', true)).rejects.toThrow()
    await expect(desktopApi.addBrowserResource(id, 'Bad', 'not a url', true)).rejects.toThrow()
  })

  it('toggles pinned and enabled flags', async () => {
    const id = (await desktopApi.saveWorkspace(draft('Work'))).workspaces[0].id
    const resourceId = (await desktopApi.addBrowserResource(id, 'A', 'a.example', true)).workspaces[0].browserResources[0].id
    const snapshot = await desktopApi.toggleBrowserResource(resourceId, false, false)
    expect(snapshot.workspaces[0].browserResources[0]).toMatchObject({ enabled: false, pinned: false })
  })
})

describe('app resources', () => {
  beforeEach(() => localStorage.clear())

  it('adds and removes an application', async () => {
    const id = (await desktopApi.saveWorkspace(draft('Work'))).workspaces[0].id
    let snapshot = await desktopApi.addAppResource(id, 'VS Code', 'C:\\VSCode\\Code.exe', 'Code', '')
    expect(snapshot.workspaces[0].appResources[0]).toMatchObject({ displayName: 'VS Code', enabled: true })
    snapshot = await desktopApi.removeAppResource(snapshot.workspaces[0].appResources[0].id)
    expect(snapshot.workspaces[0].appResources).toEqual([])
  })
})
