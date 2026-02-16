import type {
  HassBridgeInfo,
  HassRuntimeConfigPublic,
  HassUiConfig,
  HassUiPayload,
} from './types'

type JsonValue = unknown

async function readError(res: Response): Promise<string> {
  const ct = res.headers.get('content-type') || ''
  try {
    if (ct.includes('application/json')) {
      const j = (await res.json()) as { error?: string }
      return j?.error || JSON.stringify(j)
    }
  } catch {
    // ignore
  }
  try {
    const t = await res.text()
    return t || `HTTP ${res.status}`
  } catch {
    return `HTTP ${res.status}`
  }
}

export async function api<T = JsonValue>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init)
  if (!res.ok) {
    throw new Error(await readError(res))
  }
  if (res.status === 204) return null as T
  return (await res.json()) as T
}

export async function getUiPayload(): Promise<HassUiPayload> {
  return api('/bifrost/hass/ui-payload')
}

export async function getBridgeInfo(): Promise<HassBridgeInfo> {
  return api('/bifrost/hass/bridge-info')
}

export async function getRuntimeConfig(): Promise<HassRuntimeConfigPublic> {
  return api('/bifrost/hass/runtime-config')
}

export async function putRuntimeConfig(body: {
  enabled: boolean
  url: string
  sync_mode?: string
}): Promise<HassRuntimeConfigPublic> {
  return api('/bifrost/hass/runtime-config', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export async function connectRuntime(): Promise<void> {
  await api('/bifrost/hass/connect', { method: 'POST' })
}

export async function disconnectRuntime(): Promise<void> {
  await api('/bifrost/hass/disconnect', { method: 'POST' })
}

export async function putToken(token: string): Promise<void> {
  await api('/bifrost/hass/token', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ token }),
  })
}

export async function deleteToken(): Promise<void> {
  await api('/bifrost/hass/token', { method: 'DELETE' })
}

export async function postSync(): Promise<void> {
  await api('/bifrost/hass/sync', { method: 'POST' })
}

export async function postApply(): Promise<void> {
  await api('/bifrost/hass/apply', { method: 'POST' })
}

export async function postLinkButton(): Promise<void> {
  await api('/bifrost/hass/linkbutton', { method: 'POST' })
}

export async function postResetBridge(): Promise<void> {
  await api('/bifrost/hass/reset-bridge', { method: 'POST' })
}

export async function putUiConfig(config: HassUiConfig): Promise<HassUiConfig> {
  return api('/bifrost/hass/ui-config', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(config),
  })
}

export async function patchEntity(entity_id: string, patch: Record<string, unknown>): Promise<HassUiConfig> {
  return api('/bifrost/hass/entity', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ entity_id, ...patch }),
  })
}

export async function putRoomRename(room_id: string, name: string): Promise<void> {
  await api('/bifrost/hass/room', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ room_id, name }),
  })
}

export async function postRoom(name: string): Promise<void> {
  await api('/bifrost/hass/rooms', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ name }),
  })
}

export async function deleteRoom(room_id: string): Promise<void> {
  await api('/bifrost/hass/rooms', {
    method: 'DELETE',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ room_id }),
  })
}

export async function postPatinaEvent(kind: string, key?: string): Promise<void> {
  await api('/bifrost/hass/patina/event', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ kind, key }),
  })
}
