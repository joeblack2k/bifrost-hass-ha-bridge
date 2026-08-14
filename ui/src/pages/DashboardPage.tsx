import type { HassBridgeInfo, HassEntitySummary, HassRoomConfig, HassRuntimeConfigPublic, HassUiConfig } from '../lib/types'
import { draftEntities, formatTimestamp, type DraftEntity } from '../lib/layout'
import type { ViewId } from '../components/AppShell'
import { Icon } from '../components/Icon'
import { StatusBadge } from '../components/StatusBadge'

export function DashboardPage(props: {
  config: HassUiConfig
  entities: HassEntitySummary[]
  runtime: HassRuntimeConfigPublic | null
  bridge: HassBridgeInfo | null
  onNavigate: (view: ViewId) => void
}) {
  const managed = draftEntities(props.entities, props.config)
  const rooms = props.config.rooms
  const exposed = managed.filter((entity) => entity.included)
  const active = exposed.filter((entity) => entity.state === 'on')
  const unassigned = managed.filter((entity) => !entity.included)
  const connectionGood = !!props.runtime?.enabled && !!props.runtime.token_present

  return (
    <div className="page-stack">
      <section className="hero-card">
        <div className="hero-copy">
          <div className="eyebrow"><span className="eyebrow-dot" /> Local bridge workspace</div>
          <h1>Make Hue feel like home.</h1>
          <p>Build the rooms you want to see in the Hue app. Arrange Home Assistant devices in a calm, visual workspace and save when it feels right.</p>
          <div className="hero-actions">
            <button className="button button-primary" type="button" onClick={() => props.onNavigate('builder')}>
              <Icon name="room" size={17} /> Open Room Builder <Icon name="arrow-right" size={16} />
            </button>
            <button className="button button-secondary" type="button" onClick={() => props.onNavigate('inventory')}>
              Browse inventory
            </button>
          </div>
        </div>
        <div className="hero-visual" aria-hidden="true">
          <div className="hero-orbit hero-orbit-one" />
          <div className="hero-orbit hero-orbit-two" />
          <div className="hero-lamp"><Icon name="lamp" size={42} /></div>
          <div className="hero-glow" />
        </div>
      </section>

      <div className="metric-grid">
        <Metric label="Rooms" value={rooms.length} detail="Ready to arrange" icon="room" />
        <Metric label="Exposed" value={exposed.length} detail={`${active.length} currently on`} icon="lamp" tone="warm" />
        <Metric label="Home Assistant" value={managed.length} detail={`${unassigned.length} not in Hue`} icon="cloud" tone="blue" />
        <Metric label="Last sync" value={props.bridge?.sync_status.last_sync_result === 'ok' ? 'Healthy' : 'Check'} detail={formatTimestamp(props.bridge?.sync_status.last_sync_at)} icon="refresh" tone={props.bridge?.sync_status.last_sync_result === 'ok' ? 'good' : 'warn'} />
      </div>

      <section className="section-block">
        <div className="section-heading-row">
          <div>
            <div className="eyebrow">Your home</div>
            <h2>Rooms at a glance</h2>
          </div>
          <button className="text-button" type="button" onClick={() => props.onNavigate('builder')}>Edit rooms <Icon name="arrow-right" size={15} /></button>
        </div>
        <div className="room-preview-grid">
          {rooms.slice(0, 8).map((room) => <RoomPreview key={room.id} room={room} entities={managed.filter((entity) => entity.included && entity.room_id === room.id)} onOpen={() => props.onNavigate('builder')} />)}
          {!rooms.length ? <EmptyState title="No rooms yet" body="Open Room Builder to create your first room." action="Open builder" onClick={() => props.onNavigate('builder')} /> : null}
        </div>
        {rooms.length > 8 ? <div className="section-footnote">{rooms.length - 8} more rooms in Room Builder</div> : null}
      </section>

      <section className="split-section">
        <div className="info-card info-card-attention">
          <div className="info-card-icon"><Icon name="layers" size={20} /></div>
          <div className="info-card-copy">
            <div className="eyebrow">Next step</div>
            <h3>{unassigned.length ? `${unassigned.length} entities are waiting` : 'Your entities are arranged'}</h3>
            <p>{unassigned.length ? 'Drag the lights and devices you want into a room. Nothing is applied until you save.' : 'Your current Hue layout is ready. You can still fine-tune it any time.'}</p>
          </div>
          <button className="button button-secondary button-small" type="button" onClick={() => props.onNavigate('builder')}>{unassigned.length ? 'Arrange now' : 'Fine-tune'}</button>
        </div>
        <div className="info-card">
          <div className="info-card-icon"><Icon name="link" size={20} /></div>
          <div className="info-card-copy">
            <div className="eyebrow">Connection</div>
            <h3>{connectionGood ? 'Home Assistant is connected' : 'Connect Home Assistant'}</h3>
            <p>{connectionGood ? 'Bifrost is listening for live state updates.' : 'Add a runtime token under System to start syncing entities.'}</p>
          </div>
          <StatusBadge tone={connectionGood ? 'good' : 'warn'} dot>{connectionGood ? 'Live' : 'Offline'}</StatusBadge>
        </div>
      </section>
    </div>
  )
}

function Metric(props: { label: string; value: string | number; detail: string; icon: 'room' | 'lamp' | 'cloud' | 'refresh'; tone?: 'warm' | 'blue' | 'good' | 'warn' }) {
  return <div className="metric-card"><div className={`metric-icon metric-icon-${props.tone || 'neutral'}`}><Icon name={props.icon} size={18} /></div><div className="metric-label">{props.label}</div><div className="metric-value">{props.value}</div><div className="metric-detail">{props.detail}</div></div>
}

function RoomPreview(props: { room: HassRoomConfig; entities: DraftEntity[]; onOpen: () => void }) {
  const active = props.entities.filter((entity) => entity.state === 'on').length
  return <button className="room-preview-card" type="button" onClick={props.onOpen}><div className="room-preview-head"><div className="room-preview-icon"><Icon name="room" size={17} /></div><StatusBadge tone={active ? 'good' : 'neutral'}>{active ? `${active} on` : 'All off'}</StatusBadge></div><div className="room-preview-name">{props.room.name}</div><div className="room-preview-meta">{props.entities.length} {props.entities.length === 1 ? 'entity' : 'entities'}</div><div className="room-preview-lights">{props.entities.slice(0, 5).map((entity) => <span key={entity.entity_id} className={entity.state === 'on' ? 'room-light-dot room-light-dot-on' : 'room-light-dot'} title={entity.display_name}><Icon name="lamp" size={13} /></span>)}{props.entities.length > 5 ? <span className="room-light-more">+{props.entities.length - 5}</span> : null}{!props.entities.length ? <span className="room-preview-empty">Drop lights here</span> : null}</div></button>
}

function EmptyState(props: { title: string; body: string; action: string; onClick: () => void }) {
  return <div className="empty-state"><div className="empty-state-icon"><Icon name="room" size={22} /></div><h3>{props.title}</h3><p>{props.body}</p><button className="button button-secondary button-small" type="button" onClick={props.onClick}>{props.action}</button></div>
}
