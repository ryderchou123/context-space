import { afterEach, describe, expect, it, vi } from 'vitest'
import { getTabOwners, setTabOwner } from '../../../extension/src/shared'
import { installFakeChrome } from '../helpers/fake-chrome'

// BUG-031 (Medium): ownership updates were unsynchronised read-modify-write cycles. Opening
// several tabs at once lost owners, so those tabs were neither captured nor closed later.
describe('BUG-031 concurrent ownership writes are not lost', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('keeps every owner when many tabs are recorded at once', async () => {
    installFakeChrome()
    await Promise.all(Array.from({ length: 20 }, (_, id) => setTabOwner(id, { workspaceId: 'work' })))
    expect(Object.keys(await getTabOwners())).toHaveLength(20)
  })
})
