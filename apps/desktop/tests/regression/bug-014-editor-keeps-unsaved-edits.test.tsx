import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { WorkspaceEditor } from '../../src/components/WorkspaceEditor'
import type { Workspace } from '../../src/types'

const work: Workspace = { id: 'w1', name: 'Work', icon: 'briefcase', color: '#0D9488', position: 0, browserExitBehavior: 'close', appExitBehavior: 'minimize', saveSessionOnExit: true, restoreSessionOnLaunch: true, browserResources: [], appResources: [], lastSession: [], createdAt: '', updatedAt: '' }
const props = { open: true, onOpenChange: vi.fn(), onSnapshot: vi.fn(), onError: vi.fn() }

// BUG-014 (High): the dashboard refreshes every 5 s, producing a new workspace object; the editor
// reset on every refresh, wiping unsaved edits and jumping back to the General tab.
describe('BUG-014 editor keeps unsaved edits across background refreshes', () => {
  it('keeps the typed name and the selected tab when the same workspace re-renders', async () => {
    const user = userEvent.setup()
    const { rerender } = render(<WorkspaceEditor {...props} workspace={work} />)
    const name = screen.getByLabelText('Name')
    await user.clear(name)
    await user.type(name, 'Deep Work')
    rerender(<WorkspaceEditor {...props} workspace={{ ...work, updatedAt: 'refreshed' }} />)
    expect(screen.getByLabelText('Name')).toHaveValue('Deep Work')

    await user.click(screen.getByRole('button', { name: 'Switching' }))
    rerender(<WorkspaceEditor {...props} workspace={{ ...work, updatedAt: 'refreshed again' }} />)
    expect(screen.getByRole('heading', { name: 'Switching behavior' })).toBeInTheDocument()
  })

  it('still resets when a different workspace is opened', () => {
    const { rerender } = render(<WorkspaceEditor {...props} workspace={work} />)
    rerender(<WorkspaceEditor {...props} workspace={{ ...work, id: 'w2', name: 'Games' }} />)
    expect(screen.getByLabelText('Name')).toHaveValue('Games')
  })
})
