import { describe, expect, it } from 'vitest'
import { buildLaunchUrls, dedupeTabs, effectiveAppBehavior } from '../../src/core'
import type { Workspace } from '../../src/types'

const base: Workspace = { id: 'work', name: 'Work', icon: 'briefcase', color: '#0D9488', position: 0, browserExitBehavior: 'close', appExitBehavior: 'minimize', saveSessionOnExit: true, restoreSessionOnLaunch: true, browserResources: [], appResources: [], lastSession: [], createdAt: '', updatedAt: '' }
const resource = (id: string, url: string, pinned = true, enabled = true) => ({ id, workspaceId: 'work', title: id, url, pinned, enabled, position: 0 })

describe('sessions', () => {
  it('handles an empty session', () => {
    expect(dedupeTabs([])).toEqual([])
    expect(buildLaunchUrls(base, true)).toEqual([])
  })

  it('removes duplicate tabs regardless of fragment or trailing slash', () => {
    const tabs = [{ title: 'A', url: 'https://a.example/#one' }, { title: 'B', url: 'https://a.example' }, { title: 'C', url: 'https://a.example/' }]
    expect(dedupeTabs(tabs)).toEqual([tabs[0]])
  })

  it('keeps similar but different pages', () => {
    const tabs = ['https://linkedin.com', 'https://linkedin.com/jobs', 'https://linkedin.com/messages', 'https://www.linkedin.com', 'http://linkedin.com'].map((url) => ({ title: url, url }))
    expect(dedupeTabs(tabs)).toHaveLength(5)
  })

  it('drops malformed and non-web tabs', () => {
    expect(dedupeTabs([{ title: 'Bad', url: 'no spaces allowed' }, { title: 'Settings', url: 'chrome://settings' }])).toEqual([])
  })

  it('restores the last session after pinned resources, without duplicates', () => {
    const workspace = { ...base, browserResources: [resource('LinkedIn', 'https://linkedin.com')], lastSession: [{ title: 'Again', url: 'https://linkedin.com/' }, { title: 'Docs', url: 'https://docs.rs' }] }
    expect(buildLaunchUrls(workspace, true)).toEqual(['https://linkedin.com', 'https://docs.rs'])
    expect(buildLaunchUrls(workspace, false)).toEqual(['https://linkedin.com'])
  })

  it('ignores disabled and unpinned resources at launch', () => {
    const workspace = { ...base, browserResources: [resource('1', 'https://one.example'), resource('2', 'https://two.example', true, false), resource('3', 'https://three.example', false)] }
    expect(buildLaunchUrls(workspace, false)).toEqual(['https://one.example'])
  })
})

describe('app exit behavior', () => {
  it('uses the per-app override, else the workspace default', () => {
    expect(effectiveAppBehavior('keep', base)).toBe('keep')
    expect(effectiveAppBehavior(undefined, base)).toBe('minimize')
    expect(effectiveAppBehavior(undefined, { ...base, appExitBehavior: 'safe_close' })).toBe('safe_close')
  })
})
