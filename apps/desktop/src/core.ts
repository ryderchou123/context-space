import type { AppExitBehavior, SessionTab, Workspace } from './types'

export function normalizeUrl(input: string): string {
  const trimmed = input.trim()
  if (!trimmed) throw new Error('URL is required.')
  const withProtocol = /^[a-z][a-z\d+.-]*:/i.test(trimmed) ? trimmed : `https://${trimmed}`
  const url = new URL(withProtocol)
  if (!['http:', 'https:'].includes(url.protocol)) throw new Error('Only HTTP and HTTPS URLs are supported.')
  url.hash = ''
  if ((url.protocol === 'https:' && url.port === '443') || (url.protocol === 'http:' && url.port === '80')) url.port = ''
  if (url.pathname === '/') url.pathname = ''
  return url.toString().replace(/\/$/, '')
}

export function dedupeTabs(tabs: SessionTab[]): SessionTab[] {
  const seen = new Set<string>()
  return tabs.filter((tab) => {
    try {
      const key = normalizeUrl(tab.url).toLowerCase()
      if (seen.has(key)) return false
      seen.add(key)
      return true
    } catch {
      return false
    }
  })
}

export function effectiveAppBehavior(override: AppExitBehavior | undefined, workspace: Workspace): AppExitBehavior {
  return override ?? workspace.appExitBehavior
}

export function buildLaunchUrls(workspace: Workspace, restoreSession: boolean): string[] {
  const pinned = workspace.browserResources.filter((item) => item.enabled && item.pinned).map((item) => ({ title: item.title, url: item.url }))
  return dedupeTabs(restoreSession ? [...pinned, ...workspace.lastSession] : pinned).map((item) => normalizeUrl(item.url))
}
