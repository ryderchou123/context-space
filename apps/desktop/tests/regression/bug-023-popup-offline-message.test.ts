import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installFakeChrome, stubBridge } from '../helpers/fake-chrome'

const html = readFileSync(resolve(__dirname, '../../../extension/public/popup.html'), 'utf8')

async function openPopup() {
  document.documentElement.innerHTML = html.replace(/<script[^>]*><\/script>/, '')
  vi.resetModules()
  await import('../../../extension/src/popup')
  await vi.waitFor(() => expect(document.getElementById('connection')?.textContent).not.toBe('Connecting…'))
}

// BUG-023 (Medium): with the desktop app closed, the popup showed the token setup form and
// "TypeError: Failed to fetch", suggesting the token was wrong. It also claimed "Added N tabs"
// when some were duplicates.
describe('BUG-023 popup explains the real connection problem', () => {
  let browser: ReturnType<typeof installFakeChrome>
  beforeEach(async () => {
    browser = installFakeChrome([{ url: 'https://linkedin.com', title: 'LinkedIn', active: true }])
    await browser.chrome.storage.local.set({ bridgeToken: 'token' })
  })
  afterEach(() => vi.unstubAllGlobals())

  it('desktop offline: says the app is not running and does not ask for the token', async () => {
    stubBridge({}, { offline: true })
    await openPopup()
    expect(document.getElementById('connection')?.textContent).toBe('Desktop app offline')
    expect(document.getElementById('message')?.textContent).toContain('not running')
    expect(document.getElementById('setup')?.hidden).toBe(true)
  })

  it('wrong token: shows the token form with a specific message', async () => {
    stubBridge({}, { status: 401 })
    await openPopup()
    expect(document.getElementById('setup')?.hidden).toBe(false)
    expect(document.getElementById('message')?.textContent).toContain('token was rejected')
  })

  it('reports duplicates when adding the current tab', async () => {
    stubBridge({
      'GET /api/status': { activeWorkspaceId: 'w', activeWorkspaceName: 'Work', workspaces: [{ id: 'w', name: 'Work' }] },
      'POST /api/add-tabs': { ok: true, added: 0, duplicates: 1 },
    })
    await openPopup()
    document.getElementById('add-current')!.click()
    await vi.waitFor(() => expect(document.getElementById('message')?.textContent).toBe('Added 0 tabs; 1 tab already in this workspace.'))
  })
})
