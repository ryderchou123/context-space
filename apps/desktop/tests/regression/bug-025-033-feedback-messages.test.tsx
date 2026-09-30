import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from '../../src/App'
import { WorkspaceEditor } from '../../src/components/WorkspaceEditor'

// BUG-025 (Low): every toast started its own 4 s timer, so an older timer dismissed a newer
// message after as little as a few hundred milliseconds.
describe('BUG-025 a newer toast is not dismissed by an older timer', () => {
  beforeEach(() => { localStorage.clear(); vi.useFakeTimers() })
  afterEach(() => vi.useRealTimers())

  it('keeps the second message for its full duration', async () => {
    render(<App />)
    await act(async () => { await vi.advanceTimersByTimeAsync(10) })
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }))
    await act(async () => { await vi.advanceTimersByTimeAsync(3000) })
    fireEvent.click(screen.getByRole('button', { name: 'Extension setup' }))
    await act(async () => { await vi.advanceTimersByTimeAsync(2000) }) // past the first toast's 4 s
    expect(screen.getByRole('status')).toHaveTextContent('Use the bridge token')
  })
})

// BUG-033 (Medium, accessibility): while the editor is open, Radix hides the rest of the page
// from assistive technology, so validation errors shown only in the toast were never announced.
describe('BUG-033 editor errors are announced inside the dialog', () => {
  it('shows the validation error as an alert in the dialog', () => {
    render(<WorkspaceEditor open onOpenChange={vi.fn()} onSnapshot={vi.fn()} onError={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }))
    expect(screen.getByRole('dialog')).toContainElement(screen.getByRole('alert'))
    expect(screen.getByRole('alert')).toHaveTextContent('Workspace name is required.')
  })
})
