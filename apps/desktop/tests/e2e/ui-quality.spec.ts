import { expect, test, type Page } from '@playwright/test'
import { start, workspaceRecord } from './helpers'

const seed = (count: number, name = (i: number) => `Workspace ${i + 1}`) => ({
  workspaces: Array.from({ length: count }, (_, i) => workspaceRecord(`w${i}`, name(i), { position: i })),
  extensionConnected: false, bridgeToken: 'demo', bridgePort: 47651,
})

async function expectNoHorizontalScroll(page: Page) {
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
  expect(overflow).toBeLessThanOrEqual(0)
}

test('minimum window size (900x640) has no horizontal scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 640 })
  await start(page, seed(3))
  await expect(page.getByRole('heading', { name: 'Workspace 1', exact: true })).toBeVisible()
  await expectNoHorizontalScroll(page)
})

test('long workspace names stay inside their card', async ({ page }) => {
  const long = 'Quarterly planning and cross-team research workspace with a very long name'
  await start(page, seed(1, () => long))
  const cardBox = await page.getByRole('article').boundingBox()
  const titleBox = await page.getByRole('heading', { name: long }).boundingBox()
  expect(titleBox!.x + titleBox!.width).toBeLessThanOrEqual(cardBox!.x + cardBox!.width + 1)
  await expectNoHorizontalScroll(page)
})

test('many workspaces render and stay reachable', async ({ page }) => {
  await start(page, seed(40))
  await expect(page.getByRole('article')).toHaveCount(40)
  await page.getByRole('heading', { name: 'Workspace 40', exact: true }).scrollIntoViewIfNeeded()
  await expect(page.getByRole('heading', { name: 'Workspace 40', exact: true })).toBeVisible()
  await expectNoHorizontalScroll(page)
})

test('theme toggle switches between dark and light and is remembered', async ({ page }) => {
  await start(page)
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await page.getByRole('button', { name: 'Use light theme' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
})

test('keyboard users can open the create dialog and close it with Escape', async ({ page }) => {
  await start(page)
  await page.getByRole('button', { name: 'New workspace', exact: true }).focus()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('dialog')).toBeVisible()
  await expect(page.getByLabel('Name')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog')).toHaveCount(0)
})

test('every icon-only button has an accessible name', async ({ page }) => {
  await start(page, seed(2))
  const unnamed = await page.getByRole('button').evaluateAll((buttons) => buttons.filter((b) => !(b.getAttribute('aria-label') || b.textContent?.trim())).length)
  expect(unnamed).toBe(0)
})

test('debug panel opens with Ctrl+Shift+D and shows the active workspace', async ({ page }) => {
  await start(page, { ...seed(1), activeWorkspaceId: 'w0' })
  await page.keyboard.press('Control+Shift+D')
  const panel = page.getByRole('complementary', { name: 'Debug panel' })
  await expect(panel).toBeVisible()
  await expect(panel).toContainText('Workspace 1')
  await expect(panel).toContainText('Not connected')
  await page.getByRole('button', { name: 'Close debug panel' }).click()
  await expect(panel).toHaveCount(0)
})
