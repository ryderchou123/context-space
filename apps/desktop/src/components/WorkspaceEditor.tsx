import * as Dialog from '@radix-ui/react-dialog'
import { AppWindow, ExternalLink, FolderOpen, Globe2, Plus, Settings2, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { desktopApi } from '../api'
import { errorMessage, normalizeUrl } from '../core'
import type { AppSnapshot, Workspace, WorkspaceDraft } from '../types'
import { Button } from './Button'
import { WorkspaceGlyph } from './WorkspaceCard'

const icons = ['briefcase', 'book-open', 'gamepad-2', 'dumbbell', 'code-2', 'music-2']
const colors = ['#0D9488', '#2563EB', '#7C3AED', '#EA580C', '#DC2626', '#4F46E5']

function draftFor(workspace?: Workspace): WorkspaceDraft {
  if (!workspace) return { name: '', icon: 'briefcase', color: '#0D9488', browserExitBehavior: 'close', appExitBehavior: 'minimize', saveSessionOnExit: true, restoreSessionOnLaunch: true }
  const { id, name, icon, color, browserExitBehavior, appExitBehavior, saveSessionOnExit, restoreSessionOnLaunch } = workspace
  return { id, name, icon, color, browserExitBehavior, appExitBehavior, saveSessionOnExit, restoreSessionOnLaunch }
}

interface Props { workspace?: Workspace; open: boolean; onOpenChange: (open: boolean) => void; onSnapshot: (snapshot: AppSnapshot) => void; onError: (message: string) => void }

export function WorkspaceEditor({ workspace, open, onOpenChange, onSnapshot, onError }: Props) {
  const [tab, setTab] = useState<'general' | 'browser' | 'apps' | 'behavior'>('general')
  const [draft, setDraft] = useState<WorkspaceDraft>(() => draftFor(workspace))
  const [url, setUrl] = useState('')
  const [title, setTitle] = useState('')
  const [pinned, setPinned] = useState(true)
  const [appPath, setAppPath] = useState('')
  const [appName, setAppName] = useState('')
  const [processName, setProcessName] = useState('')
  const [launchArgs, setLaunchArgs] = useState('')
  // Shown inside the dialog: while it is open, Radix hides the rest of the page (including the
  // toast) from assistive technology, so errors must also be announced here (BUG-033).
  const [error, setError] = useState<string>()
  function fail(message: string) { setError(message); onError(message) }

  // Reset the form only when the dialog opens or a different workspace is chosen. The parent
  // refreshes the snapshot every few seconds, which creates a new `workspace` object each time;
  // resetting on the object wiped unsaved edits and jumped back to the General tab (BUG-014).
  const resetKey = `${workspace?.id ?? 'new'}:${open}`
  const [appliedKey, setAppliedKey] = useState(resetKey)
  if (appliedKey !== resetKey) {
    setAppliedKey(resetKey)
    setDraft(draftFor(workspace))
    setTab('general')
    setError(undefined)
  }

  /** Returns whether the action succeeded so callers only close or clear on success. */
  async function run(action: () => Promise<AppSnapshot>): Promise<boolean> {
    try { onSnapshot(await action()); setError(undefined); return true } catch (error) { fail(errorMessage(error)); return false }
  }

  async function saveGeneral() {
    if (!draft.name.trim()) return fail('Workspace name is required.')
    const saved = await run(() => desktopApi.saveWorkspace({ ...draft, name: draft.name.trim() }))
    if (saved && !workspace) onOpenChange(false)
  }

  async function addSite() {
    if (!workspace) return fail('Save the workspace before adding resources.')
    try {
      const normalized = normalizeUrl(url)
      if (await run(() => desktopApi.addBrowserResource(workspace.id, title.trim() || new URL(normalized).hostname, normalized, pinned))) { setUrl(''); setTitle('') }
    } catch (error) { fail(errorMessage(error)) }
  }

  async function browse() {
    const selected = await desktopApi.chooseExecutable()
    if (!selected) return
    setAppPath(selected)
    const filename = selected.split(/[\\/]/).pop() ?? ''
    setProcessName(filename.replace(/\.exe$/i, ''))
    if (!appName) setAppName(filename.replace(/\.exe$/i, ''))
  }

  async function addApp() {
    if (!workspace) return fail('Save the workspace before adding resources.')
    if (!appName.trim() || !appPath.trim()) return fail('Application name and executable are required.')
    if (await run(() => desktopApi.addAppResource(workspace.id, appName.trim(), appPath.trim(), processName.trim(), launchArgs.trim()))) { setAppName(''); setAppPath(''); setProcessName(''); setLaunchArgs('') }
  }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="editor-dialog" aria-describedby="workspace-editor-description">
          <div className="editor-header"><div><Dialog.Title>{workspace ? `Edit ${workspace.name}` : 'Create workspace'}</Dialog.Title><Dialog.Description id="workspace-editor-description">Set resources and safe switching behavior.</Dialog.Description></div><Dialog.Close asChild><Button variant="ghost" aria-label="Close editor"><X size={20} /></Button></Dialog.Close></div>
          <div className="editor-body">
            <nav className="editor-tabs" aria-label="Workspace settings">
              <button className={tab === 'general' ? 'selected' : ''} onClick={() => setTab('general')}><Settings2 size={18} />General</button>
              <button className={tab === 'browser' ? 'selected' : ''} onClick={() => setTab('browser')} disabled={!workspace}><Globe2 size={18} />Browser</button>
              <button className={tab === 'apps' ? 'selected' : ''} onClick={() => setTab('apps')} disabled={!workspace}><AppWindow size={18} />Applications</button>
              <button className={tab === 'behavior' ? 'selected' : ''} onClick={() => setTab('behavior')}><ExternalLink size={18} />Switching</button>
            </nav>
            <section className="editor-panel">
              {tab === 'general' && <>
                <div className="section-heading"><h3>General</h3><p>Make this workspace easy to recognize at a glance.</p></div>
                <label>Name<input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="e.g. Work" autoFocus /></label>
                <fieldset><legend>Icon</legend><div className="choice-row">{icons.map((icon) => <button key={icon} type="button" className={`icon-choice ${draft.icon === icon ? 'selected' : ''}`} onClick={() => setDraft({ ...draft, icon })} aria-label={`Use ${icon} icon`}><WorkspaceGlyph name={icon} /></button>)}</div></fieldset>
                <fieldset><legend>Accent color</legend><div className="choice-row">{colors.map((color) => <button key={color} type="button" className={`color-choice ${draft.color === color ? 'selected' : ''}`} style={{ background: color }} onClick={() => setDraft({ ...draft, color })} aria-label={`Use ${color}`} />)}</div></fieldset>
              </>}
              {tab === 'browser' && workspace && <>
                <div className="section-heading"><h3>Browser resources</h3><p>Pinned sites open every time. Saved sites remain available without auto-opening.</p></div>
                <div className="form-grid"><label>Title<input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Documentation" /></label><label>URL<input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="example.com" /></label></div>
                <label className="check-row"><input type="checkbox" checked={pinned} onChange={(e) => setPinned(e.target.checked)} />Open every time</label><Button variant="primary" onClick={addSite}><Plus size={17} />Add website</Button>
                <div className="resource-list">{workspace.browserResources.map((resource) => <div className="resource-row" key={resource.id}><div><strong>{resource.title}</strong><small>{resource.url}</small></div><label className="mini-check"><input type="checkbox" checked={resource.pinned} onChange={(e) => run(() => desktopApi.toggleBrowserResource(resource.id, resource.enabled, e.target.checked))} />Pinned</label><Button variant="ghost" aria-label={`Remove ${resource.title}`} onClick={() => run(() => desktopApi.removeBrowserResource(resource.id))}><Trash2 size={17} /></Button></div>)}</div>
              </>}
              {tab === 'apps' && workspace && <>
                <div className="section-heading"><h3>Applications</h3><p>Existing processes are reused instead of launching duplicates.</p></div>
                <div className="form-grid"><label>Display name<input value={appName} onChange={(e) => setAppName(e.target.value)} placeholder="Visual Studio Code" /></label><label>Process name<input value={processName} onChange={(e) => setProcessName(e.target.value)} placeholder="Code" /></label></div>
                <label>Executable<div className="input-action"><input value={appPath} onChange={(e) => setAppPath(e.target.value)} placeholder="C:\\...\\app.exe" /><Button onClick={browse}><FolderOpen size={17} />Browse</Button></div></label>
                <label>Launch arguments (optional)<input value={launchArgs} onChange={(e) => setLaunchArgs(e.target.value)} placeholder="--profile work" /></label><Button variant="primary" onClick={addApp}><Plus size={17} />Add application</Button>
                <div className="resource-list">{workspace.appResources.map((app) => <div className="resource-row" key={app.id}><div><strong>{app.displayName}</strong><small>{app.executablePath}</small></div><Button variant="ghost" aria-label={`Remove ${app.displayName}`} onClick={() => run(() => desktopApi.removeAppResource(app.id))}><Trash2 size={17} /></Button></div>)}</div>
              </>}
              {tab === 'behavior' && <>
                <div className="section-heading"><h3>Switching behavior</h3><p>These rules apply only when leaving this workspace.</p></div>
                <label>Browser tabs<select value={draft.browserExitBehavior} onChange={(e) => setDraft({ ...draft, browserExitBehavior: e.target.value as WorkspaceDraft['browserExitBehavior'] })}><option value="keep">Keep tabs open</option><option value="close">Close workspace tabs</option></select></label>
                <label>Desktop applications<select value={draft.appExitBehavior} onChange={(e) => setDraft({ ...draft, appExitBehavior: e.target.value as WorkspaceDraft['appExitBehavior'] })}><option value="keep">Keep apps running</option><option value="minimize">Minimize apps (recommended)</option><option value="safe_close">Safely close apps</option></select></label>
                <label className="check-row"><input type="checkbox" checked={draft.saveSessionOnExit} onChange={(e) => setDraft({ ...draft, saveSessionOnExit: e.target.checked })} />Save session when leaving</label>
                <label className="check-row"><input type="checkbox" checked={draft.restoreSessionOnLaunch} onChange={(e) => setDraft({ ...draft, restoreSessionOnLaunch: e.target.checked })} />Restore last session on launch</label>
                <div className="safety-note"><strong>Safe by design</strong><span>Context Space never force-kills a process. “Safely close” asks each app to close and preserves its unsaved-work prompts.</span></div>
              </>}
            </section>
          </div>
          <div className="editor-footer">{workspace && <Button variant="danger" onClick={async () => { if (confirm(`Delete ${workspace.name}?`) && await run(() => desktopApi.deleteWorkspace(workspace.id))) onOpenChange(false) }}><Trash2 size={17} />Delete</Button>}<p role="alert" className="editor-error">{error}</p><Dialog.Close asChild><Button>Cancel</Button></Dialog.Close><Button variant="primary" onClick={saveGeneral}>Save changes</Button></div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
