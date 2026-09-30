import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BridgeAuthError, BridgeOfflineError, api, getClientId, getTabOwners } from '../../../extension/src/shared'
import { closeUrls, handleCommand, onTabCreated, onTabRemoved, onTabReplaced, openUrls, resetActiveCacheForTests } from '../../../extension/src/tabs'
import { installFakeChrome, stubBridge } from '../helpers/fake-chrome'

// Browser extension <-> desktop bridge boundary: real extension code, fake browser, fake HTTP.
describe('extension tab service', () => {
  let browser: ReturnType<typeof installFakeChrome>
  beforeEach(() => {
    browser = installFakeChrome()
    void browser.chrome.storage.local.set({ bridgeToken: 'token' })
    resetActiveCacheForTests()
  })
  afterEach(() => vi.unstubAllGlobals())

  it('opens workspace URLs as owned background tabs', async () => {
    await openUrls('work', ['https://linkedin.com', 'https://docs.rs'])
    expect(browser.urls()).toEqual(['https://linkedin.com', 'https://docs.rs'])
    expect(Object.values(await getTabOwners())).toEqual([
      { workspaceId: 'work', sourceUrl: 'https://linkedin.com' },
      { workspaceId: 'work', sourceUrl: 'https://docs.rs' },
    ])
    expect(browser.chrome.tabs.create).toHaveBeenCalledWith({ url: 'https://linkedin.com', active: false })
  })

  it('focuses an already open tab instead of opening a duplicate', async () => {
    const existing = browser.userOpens('https://linkedin.com/')
    await openUrls('work', ['https://linkedin.com'])
    expect(browser.tabs).toHaveLength(1)
    expect(browser.chrome.tabs.update).toHaveBeenCalledWith(existing.id, { active: true })
    expect(await getTabOwners()).toEqual({}) // a tab the user opened is never adopted
  })

  it('Work -> Games: captures Work, closes only Work tabs, opens Games', async () => {
    const userTab = browser.userOpens('https://linkedin.com/')
    await openUrls('work', ['https://linkedin.com/jobs'])
    await openUrls('games', ['https://twitch.tv'])
    browser.finishLoading()
    const { calls } = stubBridge({ 'POST /api/capture': { ok: true } })

    await handleCommand({ type: 'capture_tabs', request_id: 'r1', workspace_id: 'work' })
    expect(calls[0]).toEqual({ method: 'POST', path: '/api/capture', body: { requestId: 'r1', workspaceId: 'work', tabs: [{ title: 'https://linkedin.com/jobs', url: 'https://linkedin.com/jobs' }] } })

    await handleCommand({ type: 'close_urls', workspace_id: 'work', urls: ['https://linkedin.com', 'https://linkedin.com/jobs', 'https://twitch.tv'] })
    expect(browser.urls()).toEqual([userTab.url, 'https://twitch.tv'])
  })

  it('reports tabs it closed and cleans their ownership', async () => {
    await openUrls('work', ['https://a.test'])
    browser.finishLoading()
    const closed = await closeUrls('work', ['https://a.test'])
    expect(closed).toHaveLength(1)
    expect(await getTabOwners()).toEqual({})
  })

  it('tabs opened from a workspace tab join that workspace', async () => {
    stubBridge({ 'GET /api/status': { activeWorkspaceId: 'games', workspaces: [] } })
    await openUrls('work', ['https://github.com'])
    const opener = browser.tabs[0]
    const child = browser.userOpens('https://github.com/pulls', { openerTabId: opener.id })
    await onTabCreated({ ...child } as chrome.tabs.Tab)
    expect((await getTabOwners())[String(child.id)]).toEqual({ workspaceId: 'work' })
  })

  it('never overwrites the richer owner record written by openUrls', async () => {
    stubBridge({ 'GET /api/status': { activeWorkspaceId: 'games', workspaces: [] } })
    await openUrls('work', ['https://a.test'])
    await onTabCreated({ ...browser.tabs[0] } as chrome.tabs.Tab)
    expect((await getTabOwners())[String(browser.tabs[0].id)]).toEqual({ workspaceId: 'work', sourceUrl: 'https://a.test' })
  })

  it('ownership follows a replaced (prerendered) tab and is dropped when a tab closes', async () => {
    await openUrls('work', ['https://a.test'])
    const id = browser.tabs[0].id
    await onTabReplaced(99, id)
    expect(await getTabOwners()).toEqual({ '99': { workspaceId: 'work', sourceUrl: 'https://a.test' } })
    await onTabRemoved(99)
    expect(await getTabOwners()).toEqual({})
  })

  it('desktop offline: new tabs stay unowned and API calls fail with a clear error', async () => {
    stubBridge({}, { offline: true })
    const tab = browser.userOpens('https://news.example')
    await onTabCreated({ ...tab } as chrome.tabs.Tab)
    expect(await getTabOwners()).toEqual({})
    await expect(api('/api/status')).rejects.toBeInstanceOf(BridgeOfflineError)
    await expect(handleCommand({ type: 'capture_tabs', request_id: 'r', workspace_id: 'work' })).rejects.toThrow('not running')
  })

  it('a rejected token is reported as an auth problem, not as offline', async () => {
    stubBridge({}, { status: 401 })
    await expect(api('/api/status')).rejects.toBeInstanceOf(BridgeAuthError)
  })

  it('sends the bridge token and a stable per-installation client id with every request', async () => {
    const { fetchMock } = stubBridge({ 'GET /api/status': { workspaces: [] } })
    await api('/api/status')
    await api('/api/status')
    const clientId = await getClientId()
    expect(clientId).toMatch(/^[0-9a-f-]{36}$/)
    expect(fetchMock.mock.calls[0][1]?.headers).toMatchObject({
      'X-Context-Space-Token': 'token',
      'X-Context-Space-Client': clientId,
    })
    expect(fetchMock.mock.calls[1][1]?.headers).toMatchObject({ 'X-Context-Space-Client': clientId })
  })
})
