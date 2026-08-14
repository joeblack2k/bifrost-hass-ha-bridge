import { useMemo, useRef, useState, type FormEvent } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { HassEntitySummary, HassRoomConfig, HassUiConfig } from '../lib/types'
import { addRoom, draftEntities, placement, removeRoom, renameRoom } from '../lib/layout'
import { EntityCard } from '../components/EntityCard'
import { Icon } from '../components/Icon'
import { RoomCard } from '../components/RoomCard'
import { StatusBadge } from '../components/StatusBadge'

type DomainFilter = 'all' | 'light' | 'switch' | 'binary_sensor'

export function RoomBuilderPage(props: {
  config: HassUiConfig
  entities: HassEntitySummary[]
  onChange: (next: HassUiConfig) => void
}) {
  const [query, setQuery] = useState('')
  const [domain, setDomain] = useState<DomainFilter>('all')
  const [libraryMode, setLibraryMode] = useState<'unassigned' | 'all'>('unassigned')
  const [newRoom, setNewRoom] = useState('')
  const [draggingId, setDraggingId] = useState<string | null>(null)
  const libraryRef = useRef<HTMLDivElement>(null)
  const managed = useMemo(() => draftEntities(props.entities, props.config), [props.entities, props.config])
  const rooms = props.config.rooms

  const matching = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return managed
      .filter((entity) => libraryMode === 'all' || !entity.included)
      .filter((entity) => domain === 'all' || entity.domain === domain)
      .filter((entity) => !needle || `${entity.display_name} ${entity.entity_id} ${entity.area_name || ''} ${entity.room_name}`.toLowerCase().includes(needle))
      .sort((a, b) => a.display_name.localeCompare(b.display_name))
  }, [domain, libraryMode, managed, query])

  // TanStack Virtual exposes an imperative virtualizer instance by design.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: matching.length,
    getScrollElement: () => libraryRef.current,
    estimateSize: () => 74,
    overscan: 8,
  })

  function updatePlacement(entityId: string, roomId: string | null) {
    props.onChange(placement(props.config, entityId, roomId))
  }

  function createRoom(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!newRoom.trim()) return
    props.onChange(addRoom(props.config, newRoom))
    setNewRoom('')
  }

  function handleDrop(roomId: string, entityId: string) {
    setDraggingId(null)
    updatePlacement(entityId, roomId)
  }

  function roomEntities(room: HassRoomConfig) {
    return managed.filter((entity) => entity.included && entity.room_id === room.id)
  }

  const unassigned = managed.filter((entity) => !entity.included)
  const exposed = managed.filter((entity) => entity.included)

  return (
    <div className="page-stack builder-page">
      <section className="page-intro builder-intro">
        <div><div className="eyebrow"><span className="eyebrow-dot eyebrow-dot-warm" /> Draft workspace</div><h1>Build your rooms.</h1><p>Drag devices from the library into a room. Your bridge stays unchanged until you press Save changes.</p></div>
        <div className="builder-intro-stats"><StatusBadge tone="neutral"><Icon name="layers" size={13} /> {exposed.length} in Hue</StatusBadge><StatusBadge tone={unassigned.length ? 'warn' : 'good'}><Icon name="lamp" size={13} /> {unassigned.length} waiting</StatusBadge></div>
      </section>

      <section className="builder-toolbar">
        <label className="search-field"><Icon name="search" size={17} /><span className="sr-only">Search entities</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search lights, switches, sensors…" /></label>
        <div className="filter-pills" role="group" aria-label="Entity type filter">
          {([['all', 'All'], ['light', 'Lights'], ['switch', 'Switches'], ['binary_sensor', 'Sensors']] as const).map(([value, label]) => <button key={value} className={domain === value ? 'filter-pill filter-pill-active' : 'filter-pill'} type="button" onClick={() => setDomain(value)}>{label}</button>)}
        </div>
        <label className="builder-view-select"><span className="sr-only">Library view</span><select value={libraryMode} onChange={(event) => setLibraryMode(event.target.value as 'unassigned' | 'all')}><option value="unassigned">Not in Hue</option><option value="all">All entities</option></select><Icon name="chevron-down" size={14} /></label>
        <form className="add-room-form" onSubmit={createRoom}><label className="sr-only" htmlFor="new-room-name">New room name</label><input id="new-room-name" value={newRoom} onChange={(event) => setNewRoom(event.target.value)} placeholder="New room name" /><button className="button button-secondary button-small" type="submit" disabled={!newRoom.trim()}><Icon name="plus" size={15} /> Add room</button></form>
      </section>

      <div className="builder-layout">
        <section className="entity-library panel-surface">
          <div className="panel-heading"><div><div className="eyebrow">Entity library</div><h2>{matching.length} available</h2></div><span className="panel-heading-hint">Drag to place</span></div>
          <div className="library-help"><Icon name="spark" size={15} /> On phone, use the Move to menu on each card.</div>
          <div ref={libraryRef} className="library-scroll" aria-label="Entity library">
            <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }}>
              {virtualizer.getVirtualItems().map((item) => {
                const entity = matching[item.index]
                return <div key={entity.entity_id} className="library-row" style={{ position: 'absolute', left: 0, top: 0, width: '100%', transform: `translateY(${item.start}px)` }}><EntityCard entity={entity} rooms={rooms} context="library" onDragStart={setDraggingId} onDragEnd={() => setDraggingId(null)} onMove={updatePlacement} /></div>
              })}
            </div>
            {!matching.length ? <div className="library-empty"><Icon name="search" size={22} /><h3>No matching entities</h3><p>Try a different name or switch the library to All entities.</p></div> : null}
          </div>
        </section>

        <section className="rooms-canvas" aria-label="Room layout">
          <div className="panel-heading rooms-canvas-heading"><div><div className="eyebrow">Live mockup</div><h2>Your Hue rooms</h2></div><div className="canvas-heading-meta"><span className="canvas-legend"><span className="legend-dot legend-dot-on" /> On</span><span className="canvas-legend"><span className="legend-dot" /> Off</span></div></div>
          <div className="rooms-grid">
            {rooms.map((room) => <RoomCard key={room.id} room={room} entities={roomEntities(room)} rooms={rooms} onDropEntity={handleDrop} onMove={updatePlacement} onRename={(roomId, name) => props.onChange(renameRoom(props.config, roomId, name))} onDelete={(roomId) => props.onChange(removeRoom(props.config, roomId))} />)}
          </div>
          <div className="not-in-hue-zone" onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = 'move' }} onDrop={(event) => { event.preventDefault(); const id = event.dataTransfer.getData('text/plain'); if (id) updatePlacement(id, null); setDraggingId(null) }}>
            <div className="not-in-hue-icon"><Icon name="minus" size={17} /></div><div><strong>Not in Hue</strong><span>Drop an entity here to hide it from the bridge. It stays available in the library.</span></div><StatusBadge tone="neutral">{unassigned.length}</StatusBadge>
          </div>
          {draggingId ? <div className="drag-live-region" aria-live="polite">Dragging {managed.find((entity) => entity.entity_id === draggingId)?.display_name || 'entity'}. Drop it in a room.</div> : null}
        </section>
      </div>
    </div>
  )
}
