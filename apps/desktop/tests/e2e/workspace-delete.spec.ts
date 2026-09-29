import { expect, test } from '@playwright/test'
import { addWebsite, createWorkspace, readStore, start } from './helpers'

test('Flow 5: delete a workspace with resources; nothing is left behind after restart', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Keep')
  await createWorkspace(page, 'Temporary')
  await addWebsite(page, 'Temporary', 'Temp site', 'temp.example')
  const id = (await readStore(page)).workspaces.find((w: { name: string }) => w.name === 'Temporary').id

  await page.getByRole('button', { name: 'Edit Temporary', exact: true }).click()
  page.once('dialog', (dialog) => dialog.accept())
  await page.getByRole('button', { name: 'Delete' }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Temporary', exact: true })).toHaveCount(0)
  await expect(page.getByRole('heading', { name: 'Keep', exact: true })).toBeVisible()
  // No orphaned resources: the id appears nowhere in the stored data. (SQLite cascade is
  // asserted in db.rs delete_workspace_cascades_resources_sessions_and_active_state.)
  expect(JSON.stringify(await readStore(page))).not.toContain(id)
})

test('cancelling the delete confirmation keeps the workspace', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work')
  await page.getByRole('button', { name: 'Edit Work', exact: true }).click()
  page.once('dialog', (dialog) => dialog.dismiss())
  await page.getByRole('button', { name: 'Delete' }).click()
  await expect(page.getByRole('dialog')).toBeVisible()
  await page.getByRole('button', { name: 'Close editor' }).click()
  await expect(page.getByRole('heading', { name: 'Work', exact: true })).toBeVisible()
})
