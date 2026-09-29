import { expect, test } from '@playwright/test'
import { addWebsite, card, createWorkspace, start } from './helpers'

test('Flow 2: add LinkedIn to Work and find it after reopening and restarting', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work')
  await addWebsite(page, 'Work', 'LinkedIn', 'linkedin.com/')
  await expect(card(page, 'Work')).toContainText('1 sites')
  await page.reload()
  await page.getByRole('button', { name: 'Edit Work', exact: true }).click()
  await page.getByRole('button', { name: 'Browser' }).click()
  await expect(page.getByRole('dialog').getByText('LinkedIn', { exact: true })).toBeVisible()
  await expect(page.getByRole('dialog').getByText('https://linkedin.com', { exact: true })).toBeVisible()
})

test('a duplicate URL is refused with a message and not added twice', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work')
  await addWebsite(page, 'Work', 'LinkedIn', 'https://linkedin.com')
  await page.getByRole('button', { name: 'Edit Work', exact: true }).click()
  await page.getByRole('button', { name: 'Browser' }).click()
  await page.getByLabel('URL').fill('linkedin.com/#top')
  await page.getByRole('button', { name: 'Add website' }).click()
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('already in this workspace')
  await expect(page.getByLabel('URL')).toHaveValue('linkedin.com/#top') // input kept for correction
  await expect(page.getByRole('dialog').getByText('https://linkedin.com', { exact: true })).toHaveCount(1)
})

test('a malformed URL is refused', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work')
  await page.getByRole('button', { name: 'Edit Work', exact: true }).click()
  await page.getByRole('button', { name: 'Browser' }).click()
  await page.getByLabel('URL').fill('javascript:alert(1)')
  await page.getByRole('button', { name: 'Add website' }).click()
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('Only HTTP and HTTPS')
})

test('BUG-014: unsaved edits survive the 5-second background refresh', async ({ page }) => {
  await start(page)
  await createWorkspace(page, 'Work')
  await page.getByRole('button', { name: 'Edit Work', exact: true }).click()
  await page.getByRole('button', { name: 'Browser' }).click()
  await page.getByLabel('Title').fill('Draft title')
  await page.waitForTimeout(5_600)
  await expect(page.getByLabel('Title')).toHaveValue('Draft title')
  await expect(page.getByRole('heading', { name: 'Browser resources' })).toBeVisible()
})
