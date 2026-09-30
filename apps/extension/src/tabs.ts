import { api, getTabOwners, ownerForNewTab, planOpen, removeTabOwners, selectCaptureTabs, selectOwnedTabIds, setTabOwner, tabUrl, updateTabOwners, type Status } from './shared.js'

export type Command =
  | { type: 'capture_tabs'; request_id: string; workspace_id: string }
  | { type: 'open_urls'; workspace_id: string; urls: string[] }
  | { type: 'close_urls'; workspace_id: string; urls: string[] }

async function webTabs() {
  return (await chrome.tabs.query({})).filter(tab => tabUrl(tab) !== undefined)
}

export async function captureTabs(workspaceId: string) {
  return selectCaptureTabs(await webTabs(), await getTabOwners(), workspaceId)
}

export async function openUrls(workspaceId: string, urls: string[]): Promise<void> {
  const plan = planOpen(await webTabs(), urls, await getTabOwners(), workspaceId)
  for (const reuse of plan.reuse) {
    if (reuse.claim) await setTabOwner(reuse.tabId, { workspaceId })
  }
  const focus = plan.reuse[0]
  if (focus) {
    await chrome.tabs.update(focus.tabId, { active: true })
    if (focus.windowId !== undefined) await chrome.windows.update(focus.windowId, { focused: true })
  }
  for (const url of plan.create) {
    const created = await chrome.tabs.create({ url, active: false })
    if (created.id !== undefined) await setTabOwner(created.id, { workspaceId, sourceUrl: url })
  }
}

export async function closeUrls(workspaceId: string, urls: string[]): Promise<number[]> {
  const ids = selectOwnedTabIds(await webTabs(), urls, await getTabOwners(), workspaceId)
  if (ids.length) {
    await chrome.tabs.remove(ids)
    await removeTabOwners(ids)
  }
  return ids
}

export async function handleCommand(command: Command): Promise<void> {
  if (command.type === 'capture_tabs') {
    const tabs = await captureTabs(command.workspace_id)
    await api('/api/capture', { method: 'POST', body: JSON.stringify({ requestId: command.request_id, workspaceId: command.workspace_id, tabs }) })
  } else if (command.type === 'open_urls') await openUrls(command.workspace_id, command.urls)
  else if (command.type === 'close_urls') await closeUrls(command.workspace_id, command.urls)
}

let activeCache: { value?: string; at: number } | undefined
async function activeWorkspaceId(): Promise<string | undefined> {
  if (activeCache && Date.now() - activeCache.at < 3000) return activeCache.value
  try {
    const status = await api<Status>('/api/status', {}, 1500)
    activeCache = { value: status.activeWorkspaceId, at: Date.now() }
  } catch {
    // Desktop offline: new tabs stay unowned, which means they are never closed.
    activeCache = { value: undefined, at: Date.now() }
  }
  return activeCache.value
}

export async function onTabCreated(tab: chrome.tabs.Tab): Promise<void> {
  if (tab.id === undefined) return
  const tabId = tab.id
  const workspaceId = ownerForNewTab(tab, await getTabOwners(), undefined) ?? await activeWorkspaceId()
  if (!workspaceId) return
  // openUrls records a richer owner (with sourceUrl) for tabs it creates; never overwrite it.
  await updateTabOwners(current => { current[String(tabId)] ??= { workspaceId } })
}

/** Prerendering can swap a tab for a new id; ownership follows the tab. */
export async function onTabReplaced(addedTabId: number, removedTabId: number): Promise<void> {
  await updateTabOwners(owners => {
    const owner = owners[String(removedTabId)]
    if (owner) owners[String(addedTabId)] = owner
    delete owners[String(removedTabId)]
  })
}

export function onTabRemoved(tabId: number): Promise<void> {
  return removeTabOwners([tabId])
}

export function resetActiveCacheForTests() {
  activeCache = undefined
}
