import { expect, test } from '@playwright/test'
import { card, start, workspaceRecord } from './helpers'

// Tab capture needs the extension; here the saved session is seeded and the UI flow is checked.
// Capture -> save -> reopen is covered by flow_4_* in src-tauri/src/commands.rs and by
// tests/regression/bug-008-*.
test('Flow 4: return to Work and restore its last session', async ({ page }) => {
  const lastSession = [{ title: 'LinkedIn', url: 'https://linkedin.com' }, { title: 'Docs', url: 'https://docs.rs' }]
  await start(page, { workspaces: [workspaceRecord('work', 'Work', { lastSession }), workspaceRecord('games', 'Games', { position: 1 })], extensionConnected: false, bridgeToken: 'demo', bridgePort: 47651 })
  await card(page, 'Games').getByRole('button', { name: 'Open workspace' }).click()
  await expect(page.getByRole('heading', { name: 'Last session' })).toHaveCount(0)
  await card(page, 'Work').getByRole('button', { name: 'Open workspace' }).click()
  await expect(page.getByRole('heading', { name: 'Last session' })).toBeVisible()
  await expect(page.getByText('2 saved tabs from Work')).toBeVisible()
  await page.getByRole('button', { name: 'Restore session' }).click()
  await expect(page.getByRole('status')).toContainText('Restored 2 saved tabs.')
})

test('an empty last session shows no restore prompt', async ({ page }) => {
  await start(page, { workspaces: [workspaceRecord('work', 'Work')], extensionConnected: false, bridgeToken: 'demo', bridgePort: 47651 })
  await card(page, 'Work').getByRole('button', { name: 'Open workspace' }).click()
  await expect(page.getByRole('button', { name: 'Restore session' })).toHaveCount(0)
})
