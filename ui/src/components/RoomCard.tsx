import { useState, type DragEvent } from 'react'
import clsx from 'clsx'
import type { HassRoomConfig } from '../lib/types'
import type { DraftEntity } from '../lib/layout'
import { Icon } from './Icon'
import { EntityCard } from './EntityCard'

export function RoomCard(props: {
  room: HassRoomConfig
  entities: DraftEntity[]
  rooms: HassRoomConfig[]
  onDropEntity: (roomId: string, entityId: string) => void
  onMove: (entityId: string, roomId: string | null) => void
  onRename: (roomId: string, name: string) => void
  onDelete: (roomId: string) => void
}) {
  const [over, setOver] = useState(false)

  function drop(event: DragEvent<HTMLElement>) {
    event.preventDefault()
    setOver(false)
    const entityId = event.dataTransfer.getData('text/plain')
    if (entityId) props.onDropEntity(props.room.id, entityId)
  }

  const activeCount = props.entities.filter((entity) => entity.state === 'on').length
  const isDefault = props.room.id === 'home-assistant'

  return (
    <section
      className={clsx('room-card', over && 'room-card-over')}
      data-room-drop-id={props.room.id}
      onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = 'move'; setOver(true) }}
      onDragLeave={() => setOver(false)}
      onDrop={drop}
      aria-label={`${props.room.name} room drop zone`}
    >
      <header className="room-card-header">
        <div className="room-card-title-wrap">
          <div className="room-card-icon"><Icon name="room" size={18} /></div>
          <div className="room-card-heading">
            <label className="sr-only" htmlFor={`room-name-${props.room.id}`}>Room name</label>
            <input
              key={`${props.room.id}-${props.room.name}`}
              id={`room-name-${props.room.id}`}
              className="room-name-input"
              defaultValue={props.room.name}
              onBlur={(event) => { const name = event.currentTarget.value; if (name.trim() && name.trim() !== props.room.name) props.onRename(props.room.id, name.trim()) }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') event.currentTarget.blur()
                if (event.key === 'Escape') { event.currentTarget.value = props.room.name; event.currentTarget.blur() }
              }}
            />
            <div className="room-card-meta">{props.entities.length} {props.entities.length === 1 ? 'entity' : 'entities'} · {activeCount} on</div>
          </div>
        </div>
        <div className="room-card-actions">
          <span className="room-source-label">{props.room.source_area ? 'HA area' : isDefault ? 'Default' : 'Custom'}</span>
          {!isDefault ? (
            <button className="icon-button icon-button-danger" type="button" onClick={() => props.onDelete(props.room.id)} aria-label={`Delete ${props.room.name}`} title={`Delete ${props.room.name}`}>
              <Icon name="trash" size={16} />
            </button>
          ) : null}
        </div>
      </header>
      <div className="room-card-drop-hint">
        <Icon name="plus" size={14} /> Drop entities here
      </div>
      <div className="room-card-entities">
        {props.entities.length ? props.entities.map((entity) => (
          <EntityCard key={entity.entity_id} entity={entity} rooms={props.rooms} context="room" onMove={props.onMove} />
        )) : (
          <div className="room-empty-state">No entities yet. Drag a light here or use Move to… on a card.</div>
        )}
      </div>
    </section>
  )
}
