import { useEffect, useState } from 'react'
import type { HassEntitySummary, HassRoomConfig, HassSensorKind } from '../lib/types'
import { Chip } from './Chip'
import { SelectField } from './SelectField'
import { TextField } from './TextField'
import { ToggleSwitch } from './ToggleSwitch'

export function EntityRow(props: {
  entity: HassEntitySummary
  rooms: HassRoomConfig[]
  onSetIncluded: (entity: HassEntitySummary, included: boolean) => void
  onSetRoom: (entity: HassEntitySummary, roomId: string) => void
  onSetAlias: (entity: HassEntitySummary, alias: string) => void
  onSetSensorKind: (entity: HassEntitySummary, kind: HassSensorKind) => void
  onSetSensorEnabled: (entity: HassEntitySummary, enabled: boolean) => void
}) {
  const e = props.entity

  const included = !!e.included
  const [alias, setAlias] = useState(() => e.name || '')
  useEffect(() => {
    setAlias(e.name || '')
  }, [e.name, e.entity_id])

  const caps: string[] = []
  if (e.supports_brightness) caps.push('DIM')
  if (e.supports_color) caps.push('COLOR')
  if (e.supports_color_temp) caps.push('TEMP')

  return (
    <div className="rounded-panel border border-[rgba(122,146,201,0.35)] bg-[rgba(10,18,31,0.58)] p-2.5 shadow-inset">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-[14px] font-semibold text-ink-0">{e.name}</div>
          <div className="mt-1 font-mono text-[12px] text-ink-1/70">{e.entity_id}</div>
        </div>
        <div className="flex shrink-0 flex-wrap justify-end gap-1.5">
          <Chip tone={e.available ? (e.state === 'on' ? 'good' : 'neutral') : 'warn'}>
            {e.available ? e.state : 'unavailable'}
          </Chip>
          {included ? <Chip tone="good">ADDED</Chip> : <Chip tone="bad">HIDDEN</Chip>}
        </div>
      </div>

      <div className="mt-2 grid gap-2 md:grid-cols-[190px_minmax(0,1fr)]">
        <ToggleSwitch
          checked={included}
          onChange={(v) => props.onSetIncluded(e, v)}
          label="Add to Hue app"
          wearKey={`inc:${e.entity_id}`}
        />

        <SelectField
          label="Room"
          value={e.room_id || 'home-assistant'}
          onChange={(v) => props.onSetRoom(e, v)}
          options={props.rooms.map((r) => ({ value: r.id, label: r.name }))}
        />
      </div>

      <div className="mt-2 grid gap-2 md:grid-cols-[minmax(0,1fr)_220px]">
        <TextField
          label="Hue Alias"
          value={alias}
          onChange={(v) => {
            setAlias(v)
            props.onSetAlias(e, v)
          }}
          placeholder="Name shown in Hue app"
        />

        <div className="flex items-end justify-between gap-2 rounded-control border border-[rgba(122,146,201,0.35)] bg-[rgba(16,27,44,0.82)] px-2.5 py-2">
          <div className="flex flex-wrap gap-1.5">
            {caps.length > 0 ? (
              caps.map((c) => (
                <Chip key={c} tone="neutral">
                  {c}
                </Chip>
              ))
            ) : (
              <Chip tone="neutral">ON/OFF</Chip>
            )}
          </div>
          {e.area_name ? (
            <div className="text-right text-[11px] text-ink-1">
              HA area: <span className="font-semibold">{e.area_name}</span>
            </div>
          ) : null}
        </div>
      </div>

      {e.domain === 'binary_sensor' && (
        <div className="mt-2 grid gap-2 md:grid-cols-[minmax(0,1fr)_190px]">
          <SelectField
            label="Sensor type"
            value={(e.sensor_kind || 'ignore') as string}
            onChange={(v) => props.onSetSensorKind(e, v as HassSensorKind)}
            options={[
              { value: 'motion', label: 'Motion sensor' },
              { value: 'contact', label: 'Door/contact sensor' },
              { value: 'ignore', label: 'Ignore' },
            ]}
          />
          <ToggleSwitch
            checked={!!e.enabled}
            onChange={(v) => props.onSetSensorEnabled(e, v)}
            label="Sensor enabled"
            wearKey={`sen:${e.entity_id}`}
          />
        </div>
      )}
    </div>
  )
}
