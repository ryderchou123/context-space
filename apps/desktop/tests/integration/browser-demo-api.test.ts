import { beforeEach, describe, expect, it } from 'vitest'
import { desktopApi } from '../../src/api'

const draft = { name: 'Work', icon: 'briefcase', color: '#0D9488', browserExitBehavior: 'close', appExitBehavior: 'minimize', saveSessionOnExit: true, restoreSessionOnLaunch: true } as const

describe('browser demo API integration (UI service -> persistence)', () => {
  beforeEach(() => localStorage.clear())

  it('persists workspace CRUD and browser resources across snapshots', async () => {
    let snapshot = await desktopApi.saveWorkspace(draft)
    const id = snapshot.workspaces[0].id
    snapshot = await desktopApi.addBrowserResource(id, 'LinkedIn', 'linkedin.com/', true)
    expect(snapshot.workspaces[0].browserResources[0].url).toBe('https://linkedin.com')
    await expect(desktopApi.addBrowserResource(id, 'Duplicate', 'https://linkedin.com', true)).rejects.toThrow('already')
    expect((await desktopApi.snapshot()).workspaces[0].browserResources).toHaveLength(1)
    snapshot = await desktopApi.deleteWorkspace(id)
    expect(snapshot.workspaces).toEqual([])
  })

  it('rejects switching to a deleted workspace without changing the active one', async () => {
    const id = (await desktopApi.saveWorkspace(draft)).workspaces[0].id
    await desktopApi.switchWorkspace(id)
    await expect(desktopApi.switchWorkspace('missing')).rejects.toThrow('no longer exists')
    expect((await desktopApi.snapshot()).activeWorkspaceId).toBe(id)
  })

  it('rejects saving, closing or restoring a workspace that no longer exists', async () => {
    await expect(desktopApi.saveWorkspace({ ...draft, id: 'deleted' })).rejects.toThrow('no longer exists')
    await expect(desktopApi.closeWorkspace('deleted')).rejects.toThrow('no longer exists')
    await expect(desktopApi.restoreSession('deleted')).rejects.toThrow('no longer exists')
    expect((await desktopApi.snapshot()).workspaces).toEqual([])
  })

  it('recovers from a corrupted local store instead of crashing', async () => {
    localStorage.setItem('context-space-browser-demo', '{not json')
    expect((await desktopApi.snapshot()).workspaces).toEqual([])
  })

  it('exposes debug information', async () => {
    const id = (await desktopApi.saveWorkspace(draft)).workspaces[0].id
    await desktopApi.switchWorkspace(id)
    const info = await desktopApi.debugInfo()
    expect(info).toMatchObject({ activeWorkspaceName: 'Work', extensionConnected: false })
  })
})
