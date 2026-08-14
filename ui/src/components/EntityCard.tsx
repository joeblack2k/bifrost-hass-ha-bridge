import clsx from 'clsx'
import type { DragEvent } from 'react'
import type { HassRoomConfig } from '../lib/types'
import { domainLabel, type DraftEntity, statusLabel } from '../lib/layout'
import { Icon, type IconName } from './Icon'
import { StatusBadge } from './StatusBadge'

const DOMAIN_ICON: Record<string, IconName> = {
  light: 'lamp',
  switch: 'square',
  binary_sensor: 'circle',
}

export function EntityCard(props: {
  entity: DraftEntity
  rooms: HassRoomConfig[]
  context?: 'library' | 'room' | 'inventory'
  onDragStart?: (entityId: string) => void
  onDragEnd?: () => void
  onMove: (entityId: string, roomId: string | null) => void
  onToggle?: (entityId: string, included: boolean) => void
}) {
  const entity = props.entity
  const icon = DOMAIN_ICON[entity.domain] || 'circle'

  function handleDragStart(event: DragEvent<HTMLElement>) {
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', entity.entity_id)
    props.onDragStart?.(entity.entity_id)
  }

  return (
    <article
      className={clsx('entity-card', `entity-card-${props.context || 'library'}`, !entity.available && 'entity-card-muted')}
      draggable
      onDragStart={handleDragStart}
      onDragEnd={props.onDragEnd}
      data-entity-id={entity.entity_id}
    >
      <div className="entity-card-icon"><Icon name={icon} size={17} /></div>
      <div className="entity-card-main">
        <div className="entity-card-title-row">
          <div className="entity-card-title" title={entity.display_name}>{entity.display_name}</div>
          <StatusBadge tone={!entity.available ? 'warn' : entity.state === 'on' ? 'good' : 'neutral'} dot>
            {statusLabel(entity)}
          </StatusBadge>
        </div>
        <div className="entity-card-subtitle">
          <span>{domainLabel(entity.domain)}</span>
          {entity.area_name ? <><span className="entity-card-separator">·</span><span>{entity.area_name}</span></> : null}
        </div>
      </div>
      <div className="entity-card-actions">
        {props.onToggle ? (
          <button
            className={clsx('mini-toggle', entity.included && 'mini-toggle-active')}
            type="button"
            onClick={() => props.onToggle?.(entity.entity_id, !entity.included)}
            aria-pressed={entity.included}
            aria-label={`${entity.included ? 'Remove' : 'Add'} ${entity.display_name} ${entity.included ? 'from' : 'to'} Hue`}
          >
            {entity.included ? 'Added' : 'Add'}
          </button>
        ) : null}
        <label className="move-select-label">
          <span className="sr-only">Move {entity.display_name}</span>
          <select
            className="move-select"
            value={entity.included ? entity.room_id : ''}
            onChange={(event) => props.onMove(entity.entity_id, event.target.value || null)}
            aria-label={`Move ${entity.display_name}`}
          >
            <option value="">Not in Hue</option>
            {props.rooms.map((room) => <option key={room.id} value={room.id}>{room.name}</option>)}
          </select>
        </label>
      </div>
    </article>
  )
}
