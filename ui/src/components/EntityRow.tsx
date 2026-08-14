import { useEffect, useState } from 'react'
import type {
  HassEntitySummary,
  HassLightArchetype,
  HassRoomConfig,
  HassSensorKind,
  HassSwitchMode,
} from '../lib/types'
import { Chip } from './Chip'
import { SelectField } from './SelectField'
import { TextField } from './TextField'
import { ToggleSwitch } from './ToggleSwitch'

const LIGHT_ARCHETYPE_OPTIONS: Array<{ value: HassLightArchetype; label: string }> = [
  { value: 'classic_bulb', label: 'Classic bulb' },
  { value: 'sultan_bulb', label: 'Sultan bulb' },
  { value: 'candle_bulb', label: 'Candle bulb' },
  { value: 'spot_bulb', label: 'Spot bulb' },
  { value: 'vintage_bulb', label: 'Vintage bulb' },
  { value: 'flood_bulb', label: 'Flood bulb' },
  { value: 'ceiling_round', label: 'Ceiling round' },
  { value: 'ceiling_square', label: 'Ceiling square' },
  { value: 'pendant_round', label: 'Pendant round' },
  { value: 'pendant_long', label: 'Pendant long' },
  { value: 'floor_shade', label: 'Floor shade' },
  { value: 'floor_lantern', label: 'Floor lantern' },
  { value: 'table_shade', label: 'Table shade' },
  { value: 'wall_spot', label: 'Wall spot' },
  { value: 'wall_lantern', label: 'Wall lantern' },
  { value: 'recessed_ceiling', label: 'Recessed ceiling' },
  { value: 'hue_lightstrip', label: 'Hue Lightstrip' },
  { value: 'hue_play', label: 'Hue Play' },
  { value: 'hue_go', label: 'Hue Go' },
  { value: 'hue_bloom', label: 'Hue Bloom' },
  { value: 'hue_iris', label: 'Hue Iris' },
  { value: 'hue_signe', label: 'Hue Signe' },
  { value: 'hue_tube', label: 'Hue Tube' },
];

export function EntityRow(props: {
  entity: HassEntitySummary
  rooms: HassRoomConfig[]
  onSetIncluded: (entity: HassEntitySummary, included: boolean) => void
  onSetRoom: (entity: HassEntitySummary, roomId: string) => void
  onSetAlias: (entity: HassEntitySummary, alias: string) => void
  onSetSensorKind: (entity: HassEntitySummary, kind: HassSensorKind) => void
  onSetSensorEnabled: (entity: HassEntitySummary, enabled: boolean) => void
  onSetSwitchMode: (entity: HassEntitySummary, mode: HassSwitchMode) => void
  onSetLightArchetype: (entity: HassEntitySummary, archetype: HassLightArchetype) => void
}) {
  const e = props.entity

  const included = !!e.included
  const [alias, setAlias] = useState(() => e.name || '')
  useEffect(() => {
    // This is an editable draft; syncing after the background refresh preserves focus while typing.
    // eslint-disable-next-line react-hooks/set-state-in-effect
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

      {e.domain === 'switch' && (
        <div className="mt-2 grid gap-2 md:grid-cols-[minmax(0,1fr)_190px]">
          <SelectField
            label="Hue device type"
            value={(e.switch_mode || 'plug') as HassSwitchMode}
            onChange={(v) => props.onSetSwitchMode(e, v as HassSwitchMode)}
            options={[
              { value: 'plug', label: 'Power plug' },
              { value: 'light', label: 'Light' },
            ]}
          />
          <div className="flex items-end">
            <div className="text-xs text-ink-1">
              `Plug` is excluded from Hue room light-group actions.
            </div>
          </div>
        </div>
      )}

      {e.domain === 'light' && (
        <div className="mt-2 grid gap-2 md:grid-cols-[minmax(0,1fr)_280px]">
          <SelectField
            label="Light icon"
            value={(e.light_archetype || 'classic_bulb') as HassLightArchetype}
            onChange={(v) => props.onSetLightArchetype(e, v as HassLightArchetype)}
            options={LIGHT_ARCHETYPE_OPTIONS}
          />
          <div className="flex items-end">
            <div className="text-xs text-ink-1">Changes the icon/archetype shown in Hue apps.</div>
          </div>
        </div>
      )}

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
