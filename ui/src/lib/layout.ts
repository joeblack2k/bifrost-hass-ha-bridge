import type {
  HassEntityPreference,
  HassEntitySummary,
  HassRoomConfig,
  HassUiConfig,
} from './types'

export const DEFAULT_ROOM_ID = 'home-assistant'

export type DraftEntity = HassEntitySummary & {
  display_name: string
}

export function cloneConfig(config: HassUiConfig): HassUiConfig {
  return {
    ...config,
    hidden_entity_ids: [...config.hidden_entity_ids],
    exclude_entity_ids: [...config.exclude_entity_ids],
    exclude_name_patterns: [...config.exclude_name_patterns],
    rooms: config.rooms.map((room) => ({ ...room })),
    entity_preferences: Object.fromEntries(
      Object.entries(config.entity_preferences).map(([entityId, preference]) => [
        entityId,
        { ...preference },
      ]),
    ),
    ignored_area_names: [...config.ignored_area_names],
    fake_cloud_custom: { ...config.fake_cloud_custom },
  }
}

export function roomForId(config: HassUiConfig, roomId: string | null | undefined): HassRoomConfig {
  return (
    config.rooms.find((room) => room.id === roomId) ??
    config.rooms.find((room) => room.id === DEFAULT_ROOM_ID) ?? {
      id: DEFAULT_ROOM_ID,
      name: 'Home Assistant',
      auto_created: false,
    }
  )
}

export function draftEntity(entity: HassEntitySummary, config: HassUiConfig): DraftEntity {
  const preference = config.entity_preferences[entity.entity_id]
  const room = roomForId(config, preference?.room_id || entity.room_id)
  const sensorIgnored =
    entity.domain === 'binary_sensor' &&
    (preference?.sensor_kind || entity.sensor_kind || 'ignore') === 'ignore'
  const manuallyHidden =
    preference?.visible === false ||
    config.hidden_entity_ids.some((id) => id.toLowerCase() === entity.entity_id.toLowerCase()) ||
    config.exclude_entity_ids.some((id) => id.toLowerCase() === entity.entity_id.toLowerCase())
  const included =
    !manuallyHidden &&
    !sensorIgnored &&
    (config.include_unavailable || entity.available) &&
    (preference?.visible === true || entity.included || config.default_add_new_devices_to_hue)

  return {
    ...entity,
    display_name: preference?.alias?.trim() || entity.name,
    room_id: room.id,
    room_name: room.name,
    included,
    hidden: !included,
    sensor_kind: preference?.sensor_kind || entity.sensor_kind,
    enabled: preference?.sensor_enabled ?? entity.enabled,
    switch_mode: preference?.switch_mode || entity.switch_mode,
    light_archetype: preference?.light_archetype || entity.light_archetype,
  }
}

export function draftEntities(
  entities: HassEntitySummary[],
  config: HassUiConfig,
): DraftEntity[] {
  return entities.map((entity) => draftEntity(entity, config))
}

export function placement(config: HassUiConfig, entityId: string, roomId: string | null) {
  const next = cloneConfig(config)
  const current: HassEntityPreference = next.entity_preferences[entityId] || {}
  next.entity_preferences[entityId] = {
    ...current,
    room_id: roomId || null,
    visible: roomId ? true : false,
  }
  return next
}

export function visibility(config: HassUiConfig, entityId: string, included: boolean) {
  const next = cloneConfig(config)
  const current = next.entity_preferences[entityId] || {}
  next.entity_preferences[entityId] = {
    ...current,
    visible: included,
  }
  return next
}

export function updatePreference(
  config: HassUiConfig,
  entityId: string,
  patch: HassEntityPreference,
) {
  const next = cloneConfig(config)
  next.entity_preferences[entityId] = {
    ...(next.entity_preferences[entityId] || {}),
    ...patch,
  }
  return next
}

export function roomIdForName(name: string, rooms: HassRoomConfig[]) {
  const base =
    name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'room'
  const ids = new Set(rooms.map((room) => room.id.toLowerCase()))
  let candidate = base
  let suffix = 2
  while (ids.has(candidate.toLowerCase())) {
    candidate = `${base}-${suffix}`
    suffix += 1
  }
  return candidate
}

export function addRoom(config: HassUiConfig, name: string) {
  const trimmed = name.trim()
  if (!trimmed) return config
  const next = cloneConfig(config)
  next.rooms.push({
    id: roomIdForName(trimmed, next.rooms),
    name: trimmed,
    source_area: null,
    auto_created: false,
  })
  return next
}

export function renameRoom(config: HassUiConfig, roomId: string, name: string) {
  const trimmed = name.trim()
  if (!trimmed) return config
  const next = cloneConfig(config)
  const room = next.rooms.find((candidate) => candidate.id === roomId)
  if (room) room.name = trimmed
  return next
}

export function removeRoom(config: HassUiConfig, roomId: string) {
  if (roomId === DEFAULT_ROOM_ID) return config
  const next = cloneConfig(config)
  const removed = next.rooms.find((room) => room.id === roomId)
  next.rooms = next.rooms.filter((room) => room.id !== roomId)
  if (removed?.auto_created && removed.source_area) {
    if (!next.ignored_area_names.some((name) => name.toLowerCase() === removed.source_area?.toLowerCase())) {
      next.ignored_area_names.push(removed.source_area)
    }
  }
  for (const preference of Object.values(next.entity_preferences)) {
    if (preference.room_id === roomId) preference.room_id = DEFAULT_ROOM_ID
  }
  return next
}

export function domainLabel(domain: string) {
  if (domain === 'binary_sensor') return 'Sensor'
  if (domain === 'switch') return 'Switch'
  if (domain === 'light') return 'Light'
  return domain.replaceAll('_', ' ')
}

export function statusLabel(entity: DraftEntity) {
  if (!entity.available) return 'Unavailable'
  if (entity.state === 'on') return 'On'
  if (entity.state === 'off') return 'Off'
  return entity.state
}

export function formatTimestamp(timestamp?: string | null) {
  if (!timestamp) return 'Never'
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) return timestamp
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date)
}
