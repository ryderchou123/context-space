import { afterEach, describe, expect, it, vi } from 'vitest'
import { dropLegacyOwners, getTabOwners } from '../../../extension/src/shared'
import { closeUrls, openUrls } from '../../../extension/src/tabs'
import { installFakeChrome } from '../helpers/fake-chrome'

// BUG-003 (Critical): tab ownership was stored in storage.local keyed by tab id. Chrome reuses
// tab ids after a restart, so an unrelated tab could inherit a Work tab's id and be closed.
describe('BUG-003 tab ownership does not survive a browser restart', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('an unrelated tab that reuses an old id is never closed', async () => {
    const browser = installFakeChrome()
    await openUrls('work', ['https://mail.example'])
    browser.finishLoading()
    expect(browser.tabs[0].id).toBe(1)

    // After restart the user's own tab gets id 1 and happens to show the same page.
    browser.restartBrowser(['https://mail.example'])
    expect(browser.tabs[0].id).toBe(1)
    expect(await getTabOwners()).toEqual({})
    expect(await closeUrls('work', ['https://mail.example'])).toEqual([])
    expect(browser.tabs).toHaveLength(1)
  })

  it('removes the legacy persistent ownership map on update', async () => {
    const browser = installFakeChrome()
    browser.local.workspaceTabOwners = { '1': 'work' }
    await dropLegacyOwners()
    expect(browser.local.workspaceTabOwners).toBeUndefined()
  })
})
