import { afterEach, describe, expect, it, vi } from 'vitest'
import { captureTabs, onTabCreated, openUrls, resetActiveCacheForTests } from '../../../extension/src/tabs'
import { installFakeChrome, stubBridge } from '../helpers/fake-chrome'

// BUG-008 (High): tabs the user opened while Work was active were never owned, so they were
// missing from Work's Last Session and "Restore session" did not bring them back (Flow 4).
describe('BUG-008 tabs opened during a workspace are part of its session', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('a new tab joins the active workspace and is captured', async () => {
    const browser = installFakeChrome()
    resetActiveCacheForTests()
    stubBridge({ 'GET /api/status': { activeWorkspaceId: 'work', workspaces: [] } })
    await openUrls('work', ['https://linkedin.com'])
    const extra = browser.userOpens('https://docs.rs/tauri')
    await onTabCreated({ ...extra } as chrome.tabs.Tab)
    browser.finishLoading()
    expect((await captureTabs('work')).map((tab) => tab.url)).toEqual(['https://linkedin.com', 'https://docs.rs/tauri'])
  })

  it('tabs that existed before any workspace was active stay unowned', async () => {
    const browser = installFakeChrome([{ url: 'https://bank.example', title: 'Bank' }])
    expect(await captureTabs('work')).toEqual([])
    expect(browser.tabs).toHaveLength(1)
  })
})
