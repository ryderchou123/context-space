import { describe, expect, it } from 'vitest'
import { buildLaunchUrls, dedupeTabs, effectiveAppBehavior, normalizeUrl } from './core'
import type { Workspace } from './types'

const workspace:Workspace={id:'1',name:'Work',icon:'briefcase',color:'#0D9488',position:0,browserExitBehavior:'close',appExitBehavior:'minimize',saveSessionOnExit:true,restoreSessionOnLaunch:true,createdAt:'',updatedAt:'',browserResources:[{id:'b',workspaceId:'1',title:'Example',url:'https://example.com/',pinned:true,enabled:true,position:0}],appResources:[],lastSession:[{title:'Duplicate',url:'https://example.com'},{title:'Docs',url:'https://example.com/docs#start'}]}
describe('workspace core',()=>{
  it('normalizes safe web URLs',()=>{expect(normalizeUrl('example.com/')).toBe('https://example.com');expect(()=>normalizeUrl('ftp://example.com')).toThrow()})
  it('deduplicates tabs and drops malformed values',()=>{expect(dedupeTabs([{title:'A',url:'https://a.com/'},{title:'B',url:'https://a.com'},{title:'Bad',url:'no spaces allowed'}])).toHaveLength(1)})
  it('merges pinned and restored tabs without duplicates',()=>{expect(buildLaunchUrls(workspace,true)).toEqual(['https://example.com','https://example.com/docs'])})
  it('uses per-app behavior overrides',()=>{expect(effectiveAppBehavior('keep',workspace)).toBe('keep');expect(effectiveAppBehavior(undefined,workspace)).toBe('minimize')})
})
