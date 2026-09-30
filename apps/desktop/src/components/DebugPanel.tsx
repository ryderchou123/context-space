import { Bug, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { desktopApi } from '../api'
import { errorMessage } from '../core'
import type { DebugInfo } from '../types'
import { Button } from './Button'

/** Developer panel (Ctrl+Shift+D or ?debug). Everything shown here stays on this machine. */
export function DebugPanel({ onClose }: { onClose: () => void }) {
  const [info, setInfo] = useState<DebugInfo>()
  const [error, setError] = useState<string>()

  useEffect(() => {
    let alive = true
    const load = () => desktopApi.debugInfo().then((next) => { if (alive) { setInfo(next); setError(undefined) } }, (e) => { if (alive) setError(errorMessage(e)) })
    void load()
    const timer = window.setInterval(load, 3000)
    return () => { alive = false; window.clearInterval(timer) }
  }, [])

  const rows: [string, string][] = info ? [
    ['Active workspace', info.activeWorkspaceName ? `${info.activeWorkspaceName}${info.activeWorkspaceRecovered ? ' (recovered after restart)' : ''}` : 'None'],
    ['Extension', info.extensionConnected ? `Connected: ${info.extensionClientCount} client${info.extensionClientCount === 1 ? '' : 's'} (seen ${info.extensionLastSeenSecs ?? 0}s ago)` : 'Not connected'],
    ['Database', `${info.databaseStatus} · schema v${info.schemaVersion}`],
    ['Database file', info.databasePath],
    ['Last Session tabs', String(info.lastSessionTabs)],
    ['Tracked apps', info.trackedApps.length ? info.trackedApps.map((app) => `${app.displayName} (${app.running ? 'running' : 'not running'})`).join(', ') : 'None'],
    ['Last transition', info.lastTransition ? `${info.lastTransition.event} ${info.lastTransition.detail} · ${new Date(info.lastTransition.at).toLocaleTimeString()}` : 'None yet'],
  ] : []

  return (
    <aside className="debug-panel" aria-label="Debug panel">
      <div className="debug-header"><strong><Bug size={16} /> Debug</strong><Button variant="ghost" aria-label="Close debug panel" onClick={onClose}><X size={16} /></Button></div>
      {error && <p className="debug-error">{error}</p>}
      {!info && !error && <p>Loading…</p>}
      {info && <>
        <dl>{rows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
        <h3>Recent events</h3>
        <ol className="debug-events">{info.recentEvents.length ? info.recentEvents.map((entry, index) => <li key={`${entry.at}-${index}`}><time>{new Date(entry.at).toLocaleTimeString()}</time> <b>{entry.event}</b> <span>{entry.detail}</span></li>) : <li>No events yet.</li>}</ol>
      </>}
    </aside>
  )
}
