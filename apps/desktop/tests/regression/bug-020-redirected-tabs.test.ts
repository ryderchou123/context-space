import { afterEach, describe, expect, it, vi } from 'vitest'
import { closeUrls, openUrls } from '../../../extension/src/tabs'
import { installFakeChrome } from '../helpers/fake-chrome'

// BUG-020 (High): linkedin.com redirects to www.linkedin.com/feed/. The redirected tab no longer
// matched the pinned URL, so every switch opened another LinkedIn and closing missed it.
describe('BUG-020 redirected tabs are recognised by the URL they were opened for', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('reopening reuses the redirected tab and closing finds it', async () => {
    const browser = installFakeChrome()
    browser.redirect('https://linkedin.com', 'https://www.linkedin.com/feed/')
    await openUrls('work', ['https://linkedin.com'])
    browser.finishLoading()
    await openUrls('work', ['https://linkedin.com'])
    expect(browser.urls()).toEqual(['https://www.linkedin.com/feed/'])
    expect(await closeUrls('work', ['https://linkedin.com'])).toHaveLength(1)
    expect(browser.tabs).toHaveLength(0)
  })
})
