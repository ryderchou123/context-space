import { describe, expect, it } from 'vitest'
import { normalize, selectOwnedTabIds } from '../../../extension/src/shared'

// BUG-001 (Critical): close_urls closed every tab whose URL matched, including tabs the user
// opened themselves and tabs of other workspaces.
describe('BUG-001 browser tab ownership safety', () => {
  it('closes only exact URLs owned by the workspace', () => {
    const tabs = [
      { id: 1, url: 'https://linkedin.com/' },
      { id: 2, url: 'https://linkedin.com/jobs' },
      { id: 3, url: 'https://twitch.tv/' },
      { id: 4, url: 'https://linkedin.com/' },
      { id: 5, url: 'https://linkedin.com/' },
    ]
    const owners = { '1': { workspaceId: 'work' }, '2': { workspaceId: 'work' }, '3': { workspaceId: 'games' }, '4': { workspaceId: 'games' } }
    expect(selectOwnedTabIds(tabs, ['https://linkedin.com', 'https://linkedin.com/jobs', 'https://twitch.tv'], owners, 'work')).toEqual([1, 2])
  })

  it('does not broaden matching across query, path, scheme, or www', () => {
    expect(new Set([
      normalize('https://example.com'),
      normalize('https://example.com/jobs'),
      normalize('https://example.com?account=1'),
      normalize('http://example.com'),
      normalize('https://www.example.com'),
    ]).size).toBe(5)
  })
})
