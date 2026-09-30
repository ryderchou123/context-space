import { Check, ChevronRight, CircleHelp, Copy, LayoutGrid, Moon, Plus, Power, RotateCcw, Settings, Sun, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import gsap from 'gsap'
import { desktopApi } from './api'
import { Button } from './components/Button'
import { WorkspaceCard, WorkspaceGlyph } from './components/WorkspaceCard'
import { WorkspaceEditor } from './components/WorkspaceEditor'
import { DebugPanel } from './components/DebugPanel'
import { errorMessage } from './core'
import type { AppSnapshot, Workspace } from './types'

export default function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot>({ workspaces: [], extensionConnected: false, bridgeToken: '', bridgePort: 47651 })
  const [editorOpen, setEditorOpen] = useState(false)
  const [editing, setEditing] = useState<Workspace>()
  const [loading, setLoading] = useState(true)
  const [busyId, setBusyId] = useState<string>()
  const [toast, setToast] = useState<{ message: string; error?: boolean }>()
  const [theme, setTheme] = useState<'dark' | 'light'>(() => (localStorage.getItem('theme') as 'dark' | 'light') || 'dark')
  const gridRef = useRef<HTMLDivElement>(null)
  const operationInFlight = useRef(false)
  const toastTimer = useRef<number | undefined>(undefined)
  const [debugOpen, setDebugOpen] = useState(() => new URLSearchParams(window.location.search).has('debug'))
  const active = useMemo(() => snapshot.workspaces.find((w) => w.id === snapshot.activeWorkspaceId), [snapshot])
  const editingWorkspace = editing ? snapshot.workspaces.find((workspace) => workspace.id === editing.id) ?? editing : undefined

  async function refresh() {
    try { setSnapshot(await desktopApi.snapshot()) } catch (error) { show(errorMessage(error), true) } finally { setLoading(false) }
  }
  // The desktop emits `workspace-changed` after tray switches; the interval is a fallback that
  // also picks up extension connection changes.
  useEffect(() => {
    // oxlint-disable-next-line react/set-state-in-effect
    void refresh()
    const timer = window.setInterval(refresh, 5000)
    const unlisten = desktopApi.onChanged(() => { void refresh() })
    return () => { window.clearInterval(timer); void unlisten.then((stop) => stop()) }
  }, [])
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => { if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === 'd') { event.preventDefault(); setDebugOpen((open) => !open) } }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
  useEffect(() => { document.documentElement.dataset.theme = theme; localStorage.setItem('theme', theme) }, [theme])
  useEffect(() => {
    if (!gridRef.current || matchMedia('(prefers-reduced-motion: reduce)').matches) return
    gsap.fromTo(gridRef.current.children, { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.28, stagger: 0.035, ease: 'power1.out' })
  }, [snapshot.workspaces.length])
  function show(message: string, error = false) {
    // One timer per toast; an older timer must not dismiss a newer message early.
    window.clearTimeout(toastTimer.current)
    setToast({ message, error })
    toastTimer.current = window.setTimeout(() => setToast(undefined), error ? 7000 : 4000)
  }
  async function operate(id: string, action: () => Promise<{ message: string; warnings: string[] }>) {
    if (operationInFlight.current) return
    operationInFlight.current = true
    setBusyId(id)
    try { const result = await action(); show(result.warnings.length ? `${result.message} ${result.warnings.join(' ')}` : result.message) } catch (error) { show(errorMessage(error), true) } finally { await refresh(); operationInFlight.current = false; setBusyId(undefined) }
  }
  function openEditor(workspace?: Workspace) { setEditing(workspace); setEditorOpen(true) }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark"><LayoutGrid size={20} /></div><div><strong>Context Space</strong><span>Workspace manager</span></div></div>
        <nav aria-label="Primary navigation"><button className="nav-item active"><LayoutGrid size={19} />Workspaces</button><button className="nav-item" onClick={() => show('Settings are configured per workspace in V1.')}><Settings size={19} />Settings</button></nav>
        <div className="sidebar-bottom"><div className={`connection ${snapshot.extensionConnected ? 'connected' : ''}`}><span /><div><strong>{snapshot.extensionConnected ? 'Extension connected' : 'Extension offline'}</strong><small>{snapshot.extensionConnected ? 'Chrome or Edge is ready' : 'Load the extension to capture tabs'}</small></div></div><Button variant="ghost" onClick={() => show(`Use the bridge token shown on the dashboard (port ${snapshot.bridgePort}).`)}><CircleHelp size={18} />Extension setup</Button></div>
      </aside>
      <main>
        <header className="topbar"><div className="breadcrumbs"><span>Context Space</span><ChevronRight size={15} /><strong>Workspaces</strong></div><div className="top-actions"><button className="theme-toggle" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')} aria-label={`Use ${theme === 'dark' ? 'light' : 'dark'} theme`}>{theme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}</button><Button variant="primary" onClick={() => openEditor()}><Plus size={18} />New workspace</Button></div></header>
        <div className="page-content">
          <section className="hero-row"><div><p className="eyebrow">YOUR DESKTOP, ORGANIZED</p><h1>Move between contexts<br />without losing your place.</h1><p>Launch the tabs and apps you need, then restore your last session when it is time to return.</p></div>{active && <div className="active-panel"><div className="active-panel-label"><span className="pulse-dot" />ACTIVE WORKSPACE</div><div className="active-panel-main"><span className="workspace-icon" style={{ background: active.color }}><WorkspaceGlyph name={active.icon} /></span><div><strong>{active.name}</strong><span>{active.browserResources.length} sites · {active.appResources.length} apps</span></div></div>{snapshot.activeWorkspaceRecovered && <p className="recovered-note">Restored after Context Space restarted. Switching away will not close its tabs or apps until you open it again.</p>}<div className="active-panel-actions"><Button onClick={() => operate(active.id, () => desktopApi.closeWorkspace(active.id))} disabled={Boolean(busyId)}><Power size={17} />Close workspace</Button><Button variant="ghost" onClick={() => openEditor(active)}>Edit</Button></div></div>}</section>
          <section aria-labelledby="workspaces-title"><div className="section-title"><div><h2 id="workspaces-title">My workspaces</h2><p>{snapshot.workspaces.length} contexts ready to launch</p></div></div>
            {loading ? <div className="empty-state">Loading your workspaces…</div> : snapshot.workspaces.length === 0 ? <Onboarding onCreate={() => openEditor()} /> : <div className="workspace-grid" ref={gridRef}>{snapshot.workspaces.map((workspace) => <WorkspaceCard key={workspace.id} workspace={workspace} active={workspace.id === snapshot.activeWorkspaceId} busy={Boolean(busyId)} onOpen={() => operate(workspace.id, () => desktopApi.switchWorkspace(workspace.id))} onEdit={() => openEditor(workspace)} onMove={async (direction) => { if (operationInFlight.current) return; try { setSnapshot(await desktopApi.reorderWorkspace(workspace.id, direction)) } catch (error) { show(errorMessage(error), true) } }} />)}</div>}
          </section>
          <section className="bridge-card"><div><h2>Browser extension bridge</h2><p>Everything stays on this computer. Paste this token into the Chrome or Edge extension once.</p></div><code>{snapshot.bridgeToken}</code><Button onClick={async () => { await navigator.clipboard.writeText(snapshot.bridgeToken); show('Bridge token copied.') }}><Copy size={17} />Copy token</Button></section>
          {active?.lastSession.length ? <section className="last-session"><div><RotateCcw size={20} /><div><h2>Last session</h2><p>{active.lastSession.length} saved tabs from {active.name}</p></div></div><Button onClick={() => operate(active.id, () => desktopApi.restoreSession(active.id))} disabled={Boolean(busyId)}>Restore session</Button></section> : null}
        </div>
      </main>
      <WorkspaceEditor workspace={editingWorkspace} open={editorOpen} onOpenChange={setEditorOpen} onSnapshot={setSnapshot} onError={(message) => show(message, true)} />
      {debugOpen && <DebugPanel onClose={() => setDebugOpen(false)} />}
      {toast && <div role="status" className={`toast ${toast.error ? 'toast-error' : ''}`}>{toast.error ? <X size={18} /> : <Check size={18} />}{toast.message}</div>}
    </div>
  )
}

function Onboarding({ onCreate }: { onCreate: () => void }) {
  return <div className="onboarding"><div className="onboarding-visual"><div className="mini-card one" /><div className="mini-card two" /><div className="mini-card three" /></div><p className="eyebrow">WELCOME TO CONTEXT SPACE</p><h2>Create your first workspace</h2><p>Group the websites and applications you use for one activity. You can change everything later.</p><div className="template-row">{['Work', 'Study', 'Gaming'].map((name) => <Button key={name} onClick={onCreate}>{name}</Button>)}<Button variant="primary" onClick={onCreate}><Plus size={17} />Custom</Button></div></div>
}
