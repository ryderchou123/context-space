import { afterEach, describe, expect, it, vi } from 'vitest'
import { captureTabs, openUrls } from '../../../extension/src/tabs'
import { installFakeChrome } from '../helpers/fake-chrome'

// BUG-002 (High): capturing a session saved every tab in every window, so Work's Last
// Session filled up with Games and personal tabs.
describe('BUG-002 session capture only includes the workspace tabs', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('ignores user tabs and tabs of other workspaces', async () => {
    const browser = installFakeChrome()
    browser.userOpens('https://bank.example/account')
    await openUrls('work', ['https://linkedin.com'])
    await openUrls('games', ['https://twitch.tv'])
    browser.finishLoading()
    expect((await captureTabs('work')).map((tab) => tab.url)).toEqual(['https://linkedin.com'])
  })
})
