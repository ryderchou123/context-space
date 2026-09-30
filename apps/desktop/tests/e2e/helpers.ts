import { expect, type Page } from '@playwright/test'

export const DEMO_KEY = 'context-space-browser-demo'

/**
 * Starts from a clean (or seeded) store exactly once per test. Clearing storage in every init
 * script also wiped it on reload, which made the old "persists after restart" test meaningless.
 */
export async function start(page: Page, seed?: unknown) {
  await page.addInitScript(({ key, data }) => {
    if (sessionStorage.getItem('e2e-seeded')) return
    localStorage.clear()
    if (data) localStorage.setItem(key, JSON.stringify(data))
    sessionStorage.setItem('e2e-seeded', '1')
  }, { key: DEMO_KEY, data: seed ?? null })
  await page.goto('/')
}

export const card = (page: Page, name: string) => page.getByRole('article').filter({ has: page.getByRole('heading', { name, exact: true }) })

export async function createWorkspace(page: Page, name: string, behavior?: { browser?: 'keep' | 'close'; apps?: 'keep' | 'minimize' | 'safe_close' }) {
  await page.getByRole('button', { name: 'New workspace', exact: true }).click()
  await page.getByLabel('Name').fill(name)
  if (behavior) {
    await page.getByRole('button', { name: 'Switching' }).click()
    if (behavior.browser) await page.getByLabel('Browser tabs').selectOption(behavior.browser)
    if (behavior.apps) await page.getByLabel('Desktop applications').selectOption(behavior.apps)
  }
  await page.getByRole('button', { name: 'Save changes' }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible()
}

export async function addWebsite(page: Page, workspace: string, title: string, url: string) {
  await page.getByRole('button', { name: `Edit ${workspace}`, exact: true }).click()
  await page.getByRole('button', { name: 'Browser' }).click()
  await page.getByLabel('Title').fill(title)
  await page.getByLabel('URL').fill(url)
  await page.getByRole('button', { name: 'Add website' }).click()
  await expect(page.getByRole('dialog').getByText(title, { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Close editor' }).click()
}

export const readStore = (page: Page) => page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? '{}'), DEMO_KEY)

export function workspaceRecord(id: string, name: string, extra: Record<string, unknown> = {}) {
  return { id, name, icon: 'briefcase', color: '#0D9488', position: 0, browserExitBehavior: 'close', appExitBehavior: 'minimize', saveSessionOnExit: true, restoreSessionOnLaunch: true, browserResources: [], appResources: [], lastSession: [], createdAt: '', updatedAt: '', ...extra }
}
