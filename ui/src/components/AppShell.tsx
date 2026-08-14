import clsx from 'clsx'
import type React from 'react'
import type { IconName } from './Icon'
import { Icon } from './Icon'
import { StatusBadge } from './StatusBadge'

export type ViewId = 'overview' | 'builder' | 'inventory' | 'system'

const NAV: Array<{ id: ViewId; label: string; icon: IconName; hint: string }> = [
  { id: 'overview', label: 'Overview', icon: 'grid', hint: 'Your bridge at a glance' },
  { id: 'builder', label: 'Room Builder', icon: 'room', hint: 'Arrange your Hue home' },
  { id: 'inventory', label: 'Inventory', icon: 'list', hint: 'Every Home Assistant entity' },
  { id: 'system', label: 'System', icon: 'settings', hint: 'Connection and diagnostics' },
]

export function AppShell(props: {
  view: ViewId
  onNavigate: (view: ViewId) => void
  runtimeConnected: boolean
  entityCount: number
  roomCount: number
  dirty: boolean
  saving: boolean
  onSave: () => void
  onDiscard: () => void
  onRefresh: () => void
  error?: string | null
  children: React.ReactNode
}) {
  return (
    <div className="app-shell">
      <aside className="app-sidebar">
        <div className="brand-lockup">
          <div className="brand-mark"><Icon name="spark" size={18} /></div>
          <div>
            <div className="brand-name">Bifrost</div>
            <div className="brand-caption">HA Bridge</div>
          </div>
        </div>

        <div className="sidebar-section-label">Workspace</div>
        <nav className="primary-nav" aria-label="Primary navigation">
          {NAV.map((item) => (
            <NavButton key={item.id} item={item} active={props.view === item.id} onClick={props.onNavigate} />
          ))}
        </nav>

        <div className="sidebar-spacer" />
        <div className="sidebar-status-card">
          <div className="sidebar-status-title">Bridge status</div>
          <StatusBadge tone={props.runtimeConnected ? 'good' : 'warn'} dot>
            {props.runtimeConnected ? 'Connected to Home Assistant' : 'Needs attention'}
          </StatusBadge>
          <div className="sidebar-status-meta">{props.entityCount} entities · {props.roomCount} rooms</div>
        </div>
        <div className="sidebar-footer">Bifrost 1.78 · Local network</div>
      </aside>

      <div className="app-main-column">
        <header className="topbar">
          <div className="mobile-brand brand-lockup">
            <div className="brand-mark"><Icon name="spark" size={16} /></div>
            <div className="brand-name">Bifrost</div>
          </div>
          <div className="topbar-context">
            <span className="topbar-kicker">Home Assistant Hue bridge</span>
            <span className="topbar-title">{NAV.find((item) => item.id === props.view)?.label}</span>
          </div>
          <div className="topbar-actions">
            <button className="icon-button" type="button" onClick={props.onRefresh} aria-label="Refresh bridge data" title="Refresh bridge data">
              <Icon name="refresh" size={18} />
            </button>
            {props.dirty ? (
              <div className="save-cluster">
                <span className="draft-label"><span className="draft-dot" /> Unsaved changes</span>
                <button className="button button-ghost button-small" type="button" onClick={props.onDiscard} disabled={props.saving}>Discard</button>
                <button className="button button-primary button-small" type="button" onClick={props.onSave} disabled={props.saving}>
                  <Icon name="save" size={15} /> {props.saving ? 'Saving…' : 'Save changes'}
                </button>
              </div>
            ) : (
              <StatusBadge tone={props.runtimeConnected ? 'good' : 'warn'} dot>
                {props.runtimeConnected ? 'Live' : 'Offline'}
              </StatusBadge>
            )}
          </div>
        </header>

        {props.error ? <div className="global-error" role="alert">{props.error}</div> : null}
        <main className="app-content">{props.children}</main>

        <nav className="mobile-nav" aria-label="Mobile navigation">
          {NAV.map((item) => (
            <NavButton key={item.id} item={item} active={props.view === item.id} onClick={props.onNavigate} mobile />
          ))}
        </nav>
      </div>
    </div>
  )
}

function NavButton(props: {
  item: (typeof NAV)[number]
  active: boolean
  onClick: (view: ViewId) => void
  mobile?: boolean
}) {
  return (
    <button
      className={clsx('nav-button', props.active && 'nav-button-active', props.mobile && 'mobile-nav-button')}
      type="button"
      onClick={() => props.onClick(props.item.id)}
      aria-current={props.active ? 'page' : undefined}
      title={props.item.hint}
    >
      <Icon name={props.item.icon} size={props.mobile ? 19 : 18} />
      <span>{props.item.label}</span>
    </button>
  )
}
