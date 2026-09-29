import { expect, test } from '@playwright/test'
import { createWorkspace, start } from './helpers'

test('Flow 1: create a workspace and keep it after restart', async ({ page }) => {
  await start(page)
  await expect(page.getByText('Create your first workspace')).toBeVisible()
  await createWorkspace(page, 'Work')
  await page.reload() // the browser demo persists to localStorage like the app persists to SQLite
  await expect(page.getByRole('heading', { name: 'Work', exact: true })).toBeVisible()
  await expect(page.getByText('1 contexts ready to launch')).toBeVisible()
})

test('an empty name is refused and the dialog stays open with the error', async ({ page }) => {
  await start(page)
  await page.getByRole('button', { name: 'New workspace', exact: true }).click()
  await page.getByRole('button', { name: 'Save changes' }).click()
  await expect(page.getByRole('dialog').getByRole('alert')).toHaveText('Workspace name is required.')
  await expect(page.getByRole('dialog')).toBeVisible()
})

test('renaming keeps the workspace and its settings', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work', { browser: 'keep', apps: 'safe_close' })
  await page.getByRole('button', { name: 'Edit Work', exact: true }).click()
  await page.getByLabel('Name').fill('Deep Work')
  await page.getByRole('button', { name: 'Save changes' }).click()
  await page.getByRole('button', { name: 'Close editor' }).click()
  await page.reload()
  await page.getByRole('button', { name: 'Edit Deep Work', exact: true }).click()
  await page.getByRole('button', { name: 'Switching' }).click()
  await expect(page.getByLabel('Browser tabs')).toHaveValue('keep')
  await expect(page.getByLabel('Desktop applications')).toHaveValue('safe_close')
})
