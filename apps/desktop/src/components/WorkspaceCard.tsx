import { ArrowLeft, ArrowRight, BookOpen, Briefcase, Code2, Dumbbell, ExternalLink, Gamepad2, Globe2, MoreHorizontal, Music2 } from 'lucide-react'
import type { CSSProperties } from 'react'
import type { Workspace } from '../types'
import { Button } from './Button'

interface Props { workspace: Workspace; active: boolean; onOpen: () => void; onEdit: () => void; onMove: (direction: number) => void }

export function WorkspaceCard({ workspace, active, onOpen, onEdit, onMove }: Props) {
  return (
    <article className={`workspace-card ${active ? 'workspace-card-active' : ''}`} style={{ '--workspace-color': workspace.color } as CSSProperties}>
      <div className="card-accent" />
      <div className="card-topline">
        <span className="workspace-icon" aria-hidden="true"><WorkspaceGlyph name={workspace.icon} /></span>
        {active && <span className="active-badge"><span className="pulse-dot" />Active</span>}
        <Button variant="ghost" aria-label={`Edit ${workspace.name}`} onClick={onEdit}><MoreHorizontal size={19} /></Button>
      </div>
      <div><h3>{workspace.name}</h3><p className="resource-count"><Globe2 size={15} /> {workspace.browserResources.length} sites <span>·</span> {workspace.appResources.length} apps</p></div>
      <div className="card-footer">
        <Button variant={active ? 'secondary' : 'primary'} onClick={onOpen}><ExternalLink size={17} /> {active ? 'Open again' : 'Open workspace'}</Button>
        <div className="reorder-actions">
          <Button variant="ghost" aria-label={`Move ${workspace.name} left`} onClick={() => onMove(-1)}><ArrowLeft size={16} /></Button>
          <Button variant="ghost" aria-label={`Move ${workspace.name} right`} onClick={() => onMove(1)}><ArrowRight size={16} /></Button>
        </div>
      </div>
    </article>
  )
}

export function WorkspaceGlyph({ name, size = 20 }: { name: string; size?: number }) {
  const icons: Record<string, typeof Briefcase> = { briefcase: Briefcase, 'book-open': BookOpen, 'gamepad-2': Gamepad2, dumbbell: Dumbbell, 'code-2': Code2, 'music-2': Music2 }
  const Icon = icons[name] ?? Briefcase
  return <Icon size={size} strokeWidth={1.8} />
}
