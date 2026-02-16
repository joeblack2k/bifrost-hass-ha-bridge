export type HassSensorKind = 'motion' | 'contact' | 'ignore'

export interface HassRoomConfig {
  id: string
  name: string
  source_area?: string | null
  auto_created: boolean
}

export interface HassEntityPreference {
  visible?: boolean | null
  room_id?: string | null
  alias?: string | null
  sensor_kind?: HassSensorKind | null
  sensor_enabled?: boolean | null
}

export interface HassUiConfig {
  hidden_entity_ids: string[]
  exclude_entity_ids: string[]
  exclude_name_patterns: string[]
  include_unavailable: boolean
  rooms: HassRoomConfig[]
  entity_preferences: Record<string, HassEntityPreference>
  ignored_area_names: string[]
  default_add_new_devices_to_hue: boolean
  sync_hass_areas_to_rooms: boolean
}

export interface HassEntitySummary {
  entity_id: string
  domain: 'light' | 'switch' | 'binary_sensor' | string
  name: string
  state: string
  available: boolean
  included: boolean
  hidden: boolean
  area_name?: string | null
  room_id: string
  room_name: string
  mapped_type: string
  supports_brightness: boolean
  supports_color: boolean
  supports_color_temp: boolean
  sensor_kind?: HassSensorKind | null
  enabled: boolean
}

export interface HassSyncStatus {
  last_sync_at?: string | null
  last_sync_result?: string | null
  sync_in_progress: boolean
  last_sync_duration_ms?: number | null
}

export interface HassBridgeInfo {
  bridge_name: string
  bridge_id: string
  software_version: string
  mac: string
  ipaddress: string
  netmask: string
  gateway: string
  timezone: string
  total_entities: number
  included_entities: number
  hidden_entities: number
  room_count: number
  linkbutton_active: boolean
  default_add_new_devices_to_hue: boolean
  sync_hass_areas_to_rooms: boolean
  sync_status: HassSyncStatus
}

export interface HassRuntimeConfigPublic {
  enabled: boolean
  url: string
  sync_mode: string
  token_present: boolean
}

export interface HassPatinaPublic {
  install_date: string
  interaction_count: number
  patina_level: number
  stage: 'fresh' | 'used' | 'loved'
}

export interface HassUiPayload {
  config: HassUiConfig
  entities: HassEntitySummary[]
  logs: string[]
  sync: HassSyncStatus
  patina: HassPatinaPublic
}
