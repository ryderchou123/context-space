import { render,screen } from '@testing-library/react'
import { describe,expect,it,vi } from 'vitest'
import { WorkspaceCard } from './WorkspaceCard'
import type { Workspace } from '../types'
const workspace:Workspace={id:'1',name:'Work',icon:'briefcase',color:'#0D9488',position:0,browserExitBehavior:'close',appExitBehavior:'minimize',saveSessionOnExit:true,restoreSessionOnLaunch:true,createdAt:'',updatedAt:'',browserResources:[],appResources:[],lastSession:[]}
describe('WorkspaceCard',()=>{it('shows active state and accessible actions',()=>{render(<WorkspaceCard workspace={workspace} active onOpen={vi.fn()} onEdit={vi.fn()} onMove={vi.fn()}/>);expect(screen.getByText('Active')).toBeInTheDocument();expect(screen.getByRole('button',{name:'Edit Work'})).toBeInTheDocument();expect(screen.getByRole('button',{name:'Open again'})).toBeInTheDocument()})})
