export const BASE_URL = 'http://127.0.0.1:47651'
export interface TabInfo { title: string; url: string }
export interface WorkspaceItem { id: string; name: string }
export interface Status { activeWorkspaceId?: string; activeWorkspaceName?: string; workspaces: WorkspaceItem[] }

/** The desktop app is not running or the bridge port is unreachable. */
export class BridgeOfflineError extends Error {}
/** The desktop app answered but rejected the token. */
export class BridgeAuthError extends Error {}

export async function getToken(): Promise<string> {
  const value = (await chrome.storage.local.get('bridgeToken')).bridgeToken
  return typeof value === 'string' ? value : ''
}

const CLIENT_ID_KEY = 'bridgeClientId'
export async function getClientId(): Promise<string> {
  const existing = (await chrome.storage.local.get(CLIENT_ID_KEY))[CLIENT_ID_KEY]
  if (typeof existing === 'string' && existing.length > 0) return existing
  const generated = crypto.randomUUID()
  await chrome.storage.local.set({ [CLIENT_ID_KEY]: generated })
  const stored = (await chrome.storage.local.get(CLIENT_ID_KEY))[CLIENT_ID_KEY]
  return typeof stored === 'string' && stored.length > 0 ? stored : generated
}

export async function api<T>(path: string, init: RequestInit = {}, timeoutMs?: number): Promise<T> {
  const token = await getToken()
  const clientId = await getClientId()
  let response: Response
  try {
    response = await fetch(`${BASE_URL}${path}`, {
      ...init,
      signal: timeoutMs ? AbortSignal.timeout(timeoutMs) : init.signal,
      headers: { 'Content-Type': 'application/json', 'X-Context-Space-Token': token, 'X-Context-Space-Client': clientId, ...init.headers },
    })
  } catch {
    throw new BridgeOfflineError('Context Space desktop app is not running.')
  }
  if (response.status === 401) throw new BridgeAuthError('The bridge token was rejected. Copy it again from the desktop app.')
  if (!response.ok) throw new Error((await response.json().catch(() => ({}))).error ?? `Desktop bridge returned ${response.status}`)
  return response.json() as Promise<T>
}

export interface TabCandidate { id?: number; url?: string; pendingUrl?: string; windowId?: number; title?: string; openerTabId?: number }

/** A tab that is still loading only has `pendingUrl`; ignoring it caused duplicate tabs. */
export function tabUrl(tab: TabCandidate): string | undefined {
  const url = tab.url || tab.pendingUrl
  return url && /^https?:/i.test(url) ? url : undefined
}

/**
 * URL identity shared with the desktop app (tests/fixtures/url-normalization.json).
 * Drops the fragment and a trailing slash without a query; the URL parser already drops
 * default ports and lowercases the host. Scheme, www, path case and query stay significant.
 */
export function normalize(url: string): string {
  try {
    const value = new URL(url.trim())
    if (!['http:', 'https:'].includes(value.protocol)) return url.trim()
    value.hash = ''
    const text = value.toString()
    return !text.includes('?') && text.endsWith('/') ? text.slice(0, -1) : text
  } catch {
    return url.trim()
  }
}

const OWNERS_KEY = 'workspaceTabOwners'
export interface TabOwner { workspaceId: string; sourceUrl?: string }
export type TabOwners = Record<string, TabOwner>

/** Accepts the legacy `tabId -> workspaceId` string format as well. */
export function ownerOf(owners: Record<string, unknown>, tabId: number | undefined): TabOwner | undefined {
  if (tabId === undefined) return undefined
  const value = owners[String(tabId)]
  if (typeof value === 'string') return { workspaceId: value }
  if (value && typeof value === 'object' && typeof (value as TabOwner).workspaceId === 'string') return value as TabOwner
  return undefined
}

function matchesOwned(tab: TabCandidate, owner: TabOwner | undefined, workspaceId: string, wanted: Set<string>): boolean {
  if (owner?.workspaceId !== workspaceId) return false
  const url = tabUrl(tab)
  return Boolean((url && wanted.has(normalize(url))) || (owner.sourceUrl && wanted.has(normalize(owner.sourceUrl))))
}

/**
 * Tabs to close: only tabs this workspace opened or adopted, and only when their current URL
 * or the URL they were opened for is in the list. Unowned tabs are never closed.
 */
export function selectOwnedTabIds(tabs: TabCandidate[], urls: string[], owners: TabOwners, workspaceId: string): number[] {
  const wanted = new Set(urls.map(normalize))
  return tabs.flatMap(tab => tab.id !== undefined && matchesOwned(tab, ownerOf(owners, tab.id), workspaceId, wanted) ? [tab.id] : [])
}

export function selectCaptureTabs(tabs: TabCandidate[], owners: TabOwners, workspaceId: string): TabInfo[] {
  return tabs.flatMap(tab => {
    const url = tabUrl(tab)
    if (!url || ownerOf(owners, tab.id)?.workspaceId !== workspaceId) return []
    return [{ title: tab.title || new URL(url).hostname, url }]
  })
}

export interface OpenPlan {
  reuse: { tabId: number; windowId?: number; claim: boolean }[]
  create: string[]
}

/**
 * Decides which requested URLs reuse an existing tab. Preference: a tab this workspace already
 * owns (by current or original URL, which covers redirects), then a tab another workspace owns
 * (ownership moves here), then any matching tab the user opened (stays unowned, never closed).
 */
export function planOpen(tabs: TabCandidate[], urls: string[], owners: TabOwners, workspaceId: string): OpenPlan {
  const plan: OpenPlan = { reuse: [], create: [] }
  const used = new Set<number>()
  const seen = new Set<string>()
  for (const url of urls) {
    const key = normalize(url)
    if (seen.has(key)) continue
    seen.add(key)
    const wanted = new Set([key])
    const available = tabs.filter(tab => tab.id !== undefined && !used.has(tab.id))
    const sameUrl = (tab: TabCandidate) => { const current = tabUrl(tab); return Boolean(current && normalize(current) === key) }
    const own = available.find(tab => matchesOwned(tab, ownerOf(owners, tab.id), workspaceId, wanted))
    const other = available.find(tab => sameUrl(tab) && ownerOf(owners, tab.id) !== undefined)
    const unowned = available.find(tab => sameUrl(tab) && ownerOf(owners, tab.id) === undefined)
    const match = own ?? other ?? unowned
    if (match?.id === undefined) { plan.create.push(url); continue }
    used.add(match.id)
    plan.reuse.push({ tabId: match.id, windowId: match.windowId, claim: match === other && match !== own })
  }
  return plan
}

/** New tabs join the workspace of the tab that opened them, else the active workspace. */
export function ownerForNewTab(tab: TabCandidate, owners: TabOwners, activeWorkspaceId: string | undefined): string | undefined {
  return ownerOf(owners, tab.openerTabId)?.workspaceId ?? activeWorkspaceId
}

// Ownership lives in storage.session: Chrome reuses tab ids after a browser restart, so a
// persisted map could attribute (and later close) unrelated tabs. It is cleared on restart.
export async function getTabOwners(): Promise<TabOwners> {
  const value = (await chrome.storage.session.get(OWNERS_KEY))[OWNERS_KEY]
  return value && typeof value === 'object' ? value as TabOwners : {}
}

let ownerWrites: Promise<unknown> = Promise.resolve()
/** Serializes read-modify-write updates so concurrent tab events cannot drop each other. */
export function updateTabOwners(update: (owners: TabOwners) => void): Promise<void> {
  const next = ownerWrites.then(async () => {
    const owners = await getTabOwners()
    update(owners)
    await chrome.storage.session.set({ [OWNERS_KEY]: owners })
  })
  ownerWrites = next.catch(() => undefined)
  return next
}

export function setTabOwner(tabId: number, owner: TabOwner): Promise<void> {
  return updateTabOwners(owners => { owners[String(tabId)] = owner })
}

export function removeTabOwners(tabIds: number[]): Promise<void> {
  if (!tabIds.length) return Promise.resolve()
  return updateTabOwners(owners => { for (const id of tabIds) delete owners[String(id)] })
}

/** Removes the pre-0.1.1 ownership map that was kept across browser restarts. */
export async function dropLegacyOwners(): Promise<void> {
  await chrome.storage.local.remove(OWNERS_KEY)
}
