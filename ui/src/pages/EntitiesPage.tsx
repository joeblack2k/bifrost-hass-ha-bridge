import { useMemo, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { HassEntitySummary, HassRoomConfig, HassSensorKind } from '../lib/types'
import { EntityRow } from '../components/EntityRow'

function norm(s: string) {
  return (s || '').toLowerCase()
}

export function EntitiesPage(props: {
  title: string
  subtitle: string
  entities: HassEntitySummary[]
  rooms: HassRoomConfig[]
  predicate: (e: HassEntitySummary) => boolean
  onSetIncluded: (entity: HassEntitySummary, included: boolean) => void
  onSetRoom: (entity: HassEntitySummary, roomId: string) => void
  onSetAlias: (entity: HassEntitySummary, alias: string) => void
  onSetSensorKind: (entity: HassEntitySummary, kind: HassSensorKind) => void
  onSetSensorEnabled: (entity: HassEntitySummary, enabled: boolean) => void
}) {
  const [q, setQ] = useState('')

  const filtered = useMemo(() => {
    const query = norm(q).trim()
    const list = props.entities.filter(props.predicate)
    const sorted = list.slice().sort((a, b) => {
      const ai = a.included ? 0 : 1
      const bi = b.included ? 0 : 1
      if (ai !== bi) return ai - bi
      const ar = norm(a.room_name)
      const br = norm(b.room_name)
      if (ar !== br) return ar.localeCompare(br)
      return norm(a.name).localeCompare(norm(b.name))
    })
    if (!query) return sorted
    return sorted.filter((e) => {
      const s =
        `${e.name} ${e.entity_id} ${e.room_name} ${e.area_name || ''} ${e.mapped_type}`.toLowerCase()
      return s.includes(query)
    })
  }, [props.entities, props.predicate, q])

  const parentRef = useState(() => ({ current: null as HTMLDivElement | null }))[0]
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 190,
    measureElement: (el) => el?.getBoundingClientRect().height ?? 190,
    overscan: 8,
  })

  return (
    <div className="space-y-3">
      <div>
        <div className="text-[13px] font-semibold tracking-[0.08em] text-ink-1/70 uppercase">
          {props.title}
        </div>
        <div className="mt-1 text-sm text-ink-1/70">{props.subtitle}</div>
      </div>

      <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search by name, entity_id, room, area…"
          className="w-full rounded-control border border-[rgba(122,146,201,0.35)] bg-[rgba(16,27,44,0.88)] px-2.5 py-1.5 text-[13px] text-ink-0 shadow-inset focus:outline-none focus:ring-2 focus:ring-accent-blue/60"
        />
        <div className="text-xs text-ink-1">
          Showing <span className="font-semibold text-ink-0">{filtered.length}</span>
        </div>
      </div>

      <div
        ref={(el) => {
          parentRef.current = el
        }}
        className="h-[66vh] overflow-auto rounded-panel border border-[rgba(122,146,201,0.35)] bg-[rgba(9,16,29,0.6)] p-1.5"
      >
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            position: 'relative',
          }}
        >
          {virtualizer.getVirtualItems().map((vi) => {
            const e = filtered[vi.index]
            return (
              <div
                key={e.entity_id}
                data-index={vi.index}
                ref={virtualizer.measureElement}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${vi.start}px)`,
                }}
                className="px-1 py-1"
              >
                <EntityRow
                  entity={e}
                  rooms={props.rooms}
                  onSetIncluded={props.onSetIncluded}
                  onSetRoom={props.onSetRoom}
                  onSetAlias={props.onSetAlias}
                  onSetSensorKind={props.onSetSensorKind}
                  onSetSensorEnabled={props.onSetSensorEnabled}
                />
              </div>
            )
          })}
        </div>
      </div>

      <div className="text-xs text-ink-1">Tip: use the Hidden tab to quickly re-add filtered devices.</div>
    </div>
  )
}
