import { afterEach, describe, expect, it, vi } from 'vitest'
import { openUrls } from '../../../extension/src/tabs'
import { installFakeChrome } from '../helpers/fake-chrome'

// BUG-005 (High): rapid switching / "Open again" opened LinkedIn, LinkedIn, LinkedIn... because
// tabs that were still loading have an empty `url` and were invisible to duplicate detection.
describe('BUG-005 rapid switching does not create duplicate tabs', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('reuses tabs that are still loading', async () => {
    const browser = installFakeChrome()
    for (let i = 0; i < 4; i += 1) await openUrls('work', ['https://linkedin.com', 'https://docs.rs'])
    expect(browser.urls()).toEqual(['https://linkedin.com', 'https://docs.rs'])
  })

  it('Work -> Games -> Work before anything finished loading still opens each page once', async () => {
    const browser = installFakeChrome()
    await openUrls('work', ['https://linkedin.com'])
    await openUrls('games', ['https://twitch.tv'])
    await openUrls('work', ['https://linkedin.com'])
    browser.finishLoading()
    await openUrls('work', ['https://linkedin.com/'])
    expect(browser.urls()).toEqual(['https://linkedin.com', 'https://twitch.tv'])
  })
})
