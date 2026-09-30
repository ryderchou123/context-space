import { vi } from 'vitest'

export interface FakeTab { id: number; url: string; pendingUrl?: string; title: string; windowId: number; active: boolean; openerTabId?: number }

type Store = Record<string, unknown>

function area(store: Store) {
  return {
    get: vi.fn(async (key?: string) => key === undefined ? structuredClone(store) : key in store ? { [key]: structuredClone(store[key]) } : {}),
    set: vi.fn(async (items: Store) => { Object.assign(store, structuredClone(items)) }),
    remove: vi.fn(async (key: string) => { delete store[key] }),
  }
}

/**
 * In-memory stand-in for the Chrome extension APIs the service worker uses. New tabs start
 * "loading" (empty `url`, `pendingUrl` set) exactly like Chrome, until `finishLoading()`.
 */
export function installFakeChrome(initial: Partial<FakeTab>[] = []) {
  let nextId = 1
  const tabs: FakeTab[] = []
  const local: Store = {}
  let session: Store = {}
  const make = (tab: Partial<FakeTab>): FakeTab => ({ id: nextId++, url: '', title: '', windowId: 1, active: false, ...tab })
  initial.forEach((tab) => tabs.push(make(tab)))
  const redirects = new Map<string, string>()

  const chrome = {
    tabs: {
      query: vi.fn(async (query: { active?: boolean; currentWindow?: boolean } = {}) => tabs.filter((tab) => !query.active || tab.active).map((tab) => ({ ...tab }))),
      create: vi.fn(async ({ url, active }: { url: string; active?: boolean }) => { const tab = make({ pendingUrl: url, active: Boolean(active) }); tabs.push(tab); return { ...tab } }),
      update: vi.fn(async (id: number, props: { active?: boolean }) => { const tab = tabs.find((item) => item.id === id); if (tab && props.active) tab.active = true; return tab }),
      remove: vi.fn(async (ids: number[] | number) => { for (const id of [ids].flat()) { const index = tabs.findIndex((tab) => tab.id === id); if (index >= 0) tabs.splice(index, 1) } }),
    },
    windows: { update: vi.fn(async () => ({})) },
    storage: { local: area(local), get session() { return sessionArea } },
    runtime: { sendMessage: vi.fn(async () => undefined) },
  }
  let sessionArea = area(session)
  vi.stubGlobal('chrome', chrome)

  return {
    chrome,
    tabs,
    local,
    get session() { return session },
    /** Opens a tab the way the user would (not through the extension). */
    userOpens(url: string, extra: Partial<FakeTab> = {}) { const tab = make({ url, title: url, ...extra }); tabs.push(tab); return tab },
    redirect(from: string, to: string) { redirects.set(from, to) },
    finishLoading() { for (const tab of tabs) if (tab.pendingUrl) { tab.url = redirects.get(tab.pendingUrl) ?? tab.pendingUrl; tab.title = tab.url; delete tab.pendingUrl } },
    /** Browser restart: session storage is cleared and tab ids start over. */
    restartBrowser(reopen: string[]) {
      tabs.splice(0)
      nextId = 1
      session = {}
      sessionArea = area(session)
      reopen.forEach((url) => tabs.push(make({ url, title: url })))
    },
    urls() { return tabs.map((tab) => tab.url || tab.pendingUrl) },
  }
}

/** Stubs `fetch` for the desktop bridge. `routes` maps "METHOD /path" to a JSON body or handler. */
export function stubBridge(routes: Record<string, unknown | ((body: unknown) => unknown)>, options: { offline?: boolean; status?: number } = {}) {
  const calls: { method: string; path: string; body?: unknown }[] = []
  const fetchMock = vi.fn(async (input: string, init: RequestInit = {}) => {
    if (options.offline) throw new TypeError('Failed to fetch')
    const url = new URL(input)
    const method = init.method ?? 'GET'
    const body = init.body ? JSON.parse(String(init.body)) : undefined
    calls.push({ method, path: url.pathname, body })
    if (options.status && options.status !== 200) return new Response(JSON.stringify({ error: 'Invalid bridge token.' }), { status: options.status })
    const route = routes[`${method} ${url.pathname}`]
    const result = typeof route === 'function' ? route(body) : route
    return new Response(JSON.stringify(result ?? null), { status: route === undefined ? 404 : 200 })
  })
  vi.stubGlobal('fetch', fetchMock)
  return { calls, fetchMock }
}
