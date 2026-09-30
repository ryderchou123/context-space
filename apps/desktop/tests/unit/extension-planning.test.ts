import { describe, expect, it } from 'vitest'
import { ownerForNewTab, ownerOf, planOpen, selectCaptureTabs, selectOwnedTabIds, tabUrl, type TabOwners } from '../../../extension/src/shared'

const owners: TabOwners = {
  '1': { workspaceId: 'work', sourceUrl: 'https://linkedin.com' },
  '2': { workspaceId: 'work' },
  '3': { workspaceId: 'games' },
}
const tabs = [
  { id: 1, url: 'https://www.linkedin.com/feed/', windowId: 1, title: 'Feed' },
  { id: 2, url: 'https://docs.rs/', windowId: 1, title: 'Docs' },
  { id: 3, url: 'https://twitch.tv/', windowId: 2, title: 'Twitch' },
  { id: 4, url: 'https://news.example/', windowId: 1, title: 'User tab' },
  { id: 5, url: 'chrome://settings', windowId: 1, title: 'Settings' },
]

describe('tab URL', () => {
  it('uses pendingUrl while a tab is loading and ignores non-web tabs', () => {
    expect(tabUrl({ url: '', pendingUrl: 'https://a.test' })).toBe('https://a.test')
    expect(tabUrl({ url: 'chrome://newtab' })).toBeUndefined()
    expect(tabUrl({})).toBeUndefined()
  })
})

describe('ownership', () => {
  it('reads the legacy string format', () => {
    expect(ownerOf({ '7': 'work' }, 7)).toEqual({ workspaceId: 'work' })
    expect(ownerOf({ '7': 42 }, 7)).toBeUndefined()
    expect(ownerOf({}, undefined)).toBeUndefined()
  })

  it('new tabs inherit the opener workspace, else the active workspace', () => {
    expect(ownerForNewTab({ id: 9, openerTabId: 3 }, owners, 'work')).toBe('games')
    expect(ownerForNewTab({ id: 9 }, owners, 'work')).toBe('work')
    expect(ownerForNewTab({ id: 9 }, owners, undefined)).toBeUndefined()
  })
})

describe('capture', () => {
  it('captures only web tabs owned by the workspace', () => {
    expect(selectCaptureTabs(tabs, owners, 'work')).toEqual([
      { title: 'Feed', url: 'https://www.linkedin.com/feed/' },
      { title: 'Docs', url: 'https://docs.rs/' },
    ])
    expect(selectCaptureTabs(tabs, owners, 'homework')).toEqual([])
  })
})

describe('close selection', () => {
  it('closes owned tabs by current URL or the URL they were opened for (redirects)', () => {
    expect(selectOwnedTabIds(tabs, ['https://linkedin.com', 'https://docs.rs'], owners, 'work')).toEqual([1, 2])
  })

  it('never closes unowned or other-workspace tabs, even with the same URL', () => {
    expect(selectOwnedTabIds(tabs, ['https://twitch.tv', 'https://news.example'], owners, 'work')).toEqual([])
  })

  it('does not match a broader or narrower path of the same site', () => {
    const jobs = [{ id: 1, url: 'https://linkedin.com/jobs' }, { id: 2, url: 'https://linkedin.com/messages' }]
    expect(selectOwnedTabIds(jobs, ['https://linkedin.com'], { '1': { workspaceId: 'work' }, '2': { workspaceId: 'work' } }, 'work')).toEqual([])
  })
})

describe('open plan', () => {
  it('reuses own tabs (including redirected ones) and creates the rest', () => {
    const plan = planOpen(tabs, ['https://linkedin.com', 'https://docs.rs', 'https://github.com'], owners, 'work')
    expect(plan.reuse.map((r) => r.tabId)).toEqual([1, 2])
    expect(plan.create).toEqual(['https://github.com'])
    expect(plan.reuse.every((r) => !r.claim)).toBe(true)
  })

  it('claims a tab owned by another workspace but never adopts a user tab', () => {
    const plan = planOpen(tabs, ['https://twitch.tv', 'https://news.example'], owners, 'work')
    expect(plan.reuse).toEqual([{ tabId: 3, windowId: 2, claim: true }, { tabId: 4, windowId: 1, claim: false }])
    expect(plan.create).toEqual([])
  })

  it('opens each URL once even when requested twice', () => {
    const plan = planOpen([], ['https://a.test', 'https://a.test/', 'https://a.test#x'], {}, 'work')
    expect(plan.create).toEqual(['https://a.test'])
  })

  it('never reuses the same tab for two different URLs', () => {
    const plan = planOpen([{ id: 1, url: 'https://www.linkedin.com/feed' }], ['https://linkedin.com', 'https://www.linkedin.com/feed'], { '1': { workspaceId: 'work', sourceUrl: 'https://linkedin.com' } }, 'work')
    expect(plan.reuse).toHaveLength(1)
    expect(plan.create).toEqual(['https://www.linkedin.com/feed'])
  })
})
