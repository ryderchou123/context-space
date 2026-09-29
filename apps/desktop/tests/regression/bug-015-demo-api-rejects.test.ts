import { beforeEach, describe, expect, it } from 'vitest'
import { desktopApi } from '../../src/api'

// BUG-015 (Medium): the browser demo API threw synchronously instead of returning a rejected
// promise, so `.catch()` / `rejects` never saw the error (the integration suite failed 2/2).
describe('BUG-015 demo API reports errors as rejected promises', () => {
  beforeEach(() => localStorage.clear())

  it('returns a promise that rejects, never throws synchronously', async () => {
    let result: Promise<unknown> | undefined
    expect(() => { result = desktopApi.switchWorkspace('missing') }).not.toThrow()
    await expect(result).rejects.toThrow('no longer exists')
  })
})
