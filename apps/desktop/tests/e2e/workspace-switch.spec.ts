import { expect, test } from '@playwright/test'
import { card, createWorkspace, readStore, start } from './helpers'

// The browser build has no real tabs or apps. Capture/close/minimize/launch for this flow are
// verified against the real bridge in src-tauri/src/commands.rs (flow_3_* and regression_bug_*).
test('Flow 3: Work -> Games makes Games the only active workspace', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work', { browser: 'close', apps: 'minimize' })
  await createWorkspace(page, 'Games')
  await card(page, 'Work').getByRole('button', { name: 'Open workspace' }).click()
  await expect(card(page, 'Work').getByText('Active')).toBeVisible()
  await expect(page.getByText('ACTIVE WORKSPACE')).toBeVisible()

  await card(page, 'Games').getByRole('button', { name: 'Open workspace' }).click()
  await expect(card(page, 'Games').getByText('Active')).toBeVisible()
  await expect(card(page, 'Work').getByText('Active')).toHaveCount(0)
  await expect(page.getByRole('status')).toContainText('Games is now active.')
  const store = await readStore(page)
  expect(store.workspaces.find((w: { id: string }) => w.id === store.activeWorkspaceId).name).toBe('Games')
})

test('double-clicking and rapid switching end in one consistent active workspace', async ({ page }) => {
  await start(page)
  for (const name of ['Work', 'Games', 'Homework']) await createWorkspace(page, name)
  await card(page, 'Work').getByRole('button', { name: 'Open workspace' }).dblclick()
  await card(page, 'Games').getByRole('button', { name: 'Open workspace' }).click()
  await card(page, 'Homework').getByRole('button', { name: 'Open workspace' }).click()
  await card(page, 'Work').getByRole('button', { name: /Open/ }).click()
  await expect(page.getByText('Active', { exact: true })).toHaveCount(1)
  await page.reload()
  await expect(page.getByText('Active', { exact: true })).toHaveCount(1)
})

test('closing the active workspace clears the active indicator', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work')
  await card(page, 'Work').getByRole('button', { name: 'Open workspace' }).click()
  await page.getByRole('button', { name: 'Close workspace' }).click()
  await expect(page.getByText('ACTIVE WORKSPACE')).toHaveCount(0)
  await expect(card(page, 'Work').getByRole('button', { name: 'Open workspace' })).toBeVisible()
})
