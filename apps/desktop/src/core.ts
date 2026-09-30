import type { AppExitBehavior, SessionTab, Workspace } from './types'

export function normalizeUrl(input: string): string {
  const trimmed = input.trim()
  if (!trimmed) throw new Error('URL is required.')
  // "localhost:3000" is a host and port, not a "localhost:" scheme.
  const hasScheme = /^[a-z][a-z\d+.-]*:(?!\d)/i.test(trimmed)
  const withProtocol = hasScheme ? trimmed : `https://${trimmed}`
  const url = new URL(withProtocol)
  if (!['http:', 'https:'].includes(url.protocol)) throw new Error('Only HTTP and HTTPS URLs are supported.')
  // Shared rules: tests/fixtures/url-normalization.json. The parser already drops default
  // ports and lowercases the host; scheme, www, path case and query stay significant.
  url.hash = ''
  const text = url.toString()
  return !text.includes('?') && text.endsWith('/') ? text.slice(0, -1) : text
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

/** Tauri rejects with plain strings, the browser demo with Error objects. */
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
