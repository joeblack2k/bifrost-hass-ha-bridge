import { useMemo, useState } from 'react'
import type { HassEntitySummary, HassLightArchetype, HassSensorKind, HassSwitchMode, HassUiConfig } from '../lib/types'
import { domainLabel, draftEntities, updatePreference, type DraftEntity } from '../lib/layout'
import { EntityCard } from '../components/EntityCard'
import { Icon } from '../components/Icon'
import { StatusBadge } from '../components/StatusBadge'

const LIGHT_ARCHETYPES: Array<{ value: HassLightArchetype; label: string }> = [
  ['classic_bulb', 'Classic bulb'], ['sultan_bulb', 'Sultan bulb'], ['candle_bulb', 'Candle bulb'], ['spot_bulb', 'Spot bulb'], ['vintage_bulb', 'Vintage bulb'], ['flood_bulb', 'Flood bulb'], ['ceiling_round', 'Ceiling round'], ['ceiling_square', 'Ceiling square'], ['pendant_round', 'Pendant round'], ['pendant_long', 'Pendant long'], ['floor_shade', 'Floor shade'], ['floor_lantern', 'Floor lantern'], ['table_shade', 'Table shade'], ['wall_spot', 'Wall spot'], ['wall_lantern', 'Wall lantern'], ['recessed_ceiling', 'Recessed ceiling'], ['hue_lightstrip', 'Hue Lightstrip'], ['hue_play', 'Hue Play'], ['hue_go', 'Hue Go'], ['hue_bloom', 'Hue Bloom'], ['hue_iris', 'Hue Iris'], ['hue_signe', 'Hue Signe'], ['hue_tube', 'Hue Tube'],
  ].map(([value, label]) => ({ value: value as HassLightArchetype, label }))

export function InventoryPage(props: { config: HassUiConfig; entities: HassEntitySummary[]; onChange: (next: HassUiConfig) => void }) {
  const [query, setQuery] = useState('')
  const [domain, setDomain] = useState('all')
  const [onlyIncluded, setOnlyIncluded] = useState(false)
  const managed = useMemo(() => draftEntities(props.entities, props.config), [props.config, props.entities])
  const filtered = managed.filter((entity) => {
    const needle = query.trim().toLowerCase()
    return (domain === 'all' || entity.domain === domain) && (!onlyIncluded || entity.included) && (!needle || `${entity.display_name} ${entity.entity_id} ${entity.room_name} ${entity.area_name || ''}`.toLowerCase().includes(needle))
  })

  function patch(entityId: string, value: Parameters<typeof updatePreference>[2]) {
    props.onChange(updatePreference(props.config, entityId, value))
  }

  return <div className="page-stack"><section className="page-intro"><div><div className="eyebrow">Home Assistant inventory</div><h1>Every entity, one clear list.</h1><p>Use this view for precise controls. For the visual layout, switch back to Room Builder.</p></div><StatusBadge tone="neutral"><Icon name="layers" size={13} /> {filtered.length} shown</StatusBadge></section><section className="inventory-toolbar"><label className="search-field"><Icon name="search" size={17} /><span className="sr-only">Search inventory</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search by name, entity ID, room or area…" /></label><label className="builder-view-select"><span className="sr-only">Entity domain</span><select value={domain} onChange={(event) => setDomain(event.target.value)}><option value="all">All types</option><option value="light">Lights</option><option value="switch">Switches</option><option value="binary_sensor">Sensors</option><option value="scene">Scenes</option></select><Icon name="chevron-down" size={14} /></label><button className={onlyIncluded ? 'filter-pill filter-pill-active' : 'filter-pill'} type="button" onClick={() => setOnlyIncluded((value) => !value)}>{onlyIncluded ? 'Only in Hue' : 'Show all'}</button></section><section className="inventory-list panel-surface"><div className="inventory-list-head"><span>Name</span><span>Type</span><span>Room / controls</span></div>{filtered.map((entity) => <InventoryRow key={entity.entity_id} entity={entity} rooms={props.config.rooms} onMove={(entityId, roomId) => props.onChange(updatePreference(props.config, entityId, { room_id: roomId, visible: !!roomId }))} onToggle={(entityId, included) => props.onChange(updatePreference(props.config, entityId, { visible: included }))} onPatch={patch} />)}{!filtered.length ? <div className="library-empty"><Icon name="search" size={22} /><h3>No entities found</h3><p>Try a broader search or remove the filter.</p></div> : null}</section></div>
}

function InventoryRow(props: { entity: DraftEntity; rooms: HassUiConfig['rooms']; onMove: (id: string, roomId: string | null) => void; onToggle: (id: string, included: boolean) => void; onPatch: (id: string, value: Parameters<typeof updatePreference>[2]) => void }) {
  const [open, setOpen] = useState(false)
  const e = props.entity
  return <div className="inventory-row"><EntityCard entity={e} rooms={props.rooms} context="inventory" onMove={props.onMove} onToggle={props.onToggle} /><button className="inventory-details-toggle" type="button" onClick={() => setOpen((value) => !value)} aria-expanded={open}>{open ? 'Hide controls' : 'Details'} <Icon name={open ? 'chevron-down' : 'chevron-right'} size={14} /></button>{open ? <div className="inventory-details"><label>Hue alias<input defaultValue={e.display_name} onBlur={(event) => props.onPatch(e.entity_id, { alias: event.target.value })} /></label>{e.domain === 'switch' ? <label>Hue type<select value={e.switch_mode || 'plug'} onChange={(event) => props.onPatch(e.entity_id, { switch_mode: event.target.value as HassSwitchMode })}><option value="plug">Power plug</option><option value="light">Light</option></select></label> : null}{e.domain === 'light' || (e.domain === 'switch' && e.switch_mode === 'light') ? <label>Light archetype<select value={e.light_archetype || 'classic_bulb'} onChange={(event) => props.onPatch(e.entity_id, { light_archetype: event.target.value as HassLightArchetype })}>{LIGHT_ARCHETYPES.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label> : null}{e.domain === 'binary_sensor' ? <><label>Sensor type<select value={e.sensor_kind || 'ignore'} onChange={(event) => props.onPatch(e.entity_id, { sensor_kind: event.target.value as HassSensorKind })}><option value="motion">Motion</option><option value="contact">Contact</option><option value="ignore">Ignore</option></select></label><label className="checkbox-field"><input type="checkbox" checked={e.enabled} onChange={(event) => props.onPatch(e.entity_id, { sensor_enabled: event.target.checked })} /> Sensor enabled</label></> : null}<div className="inventory-entity-id"><Icon name="link" size={13} /> {e.entity_id} · {domainLabel(e.domain)}</div></div> : null}</div>
}
