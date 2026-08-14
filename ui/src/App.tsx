import { useEffect, useMemo, useState } from 'react'
import { postPatinaEvent, postSync, putUiConfig } from './lib/api'
import type { HassUiConfig } from './lib/types'
import { AppShell, type ViewId } from './components/AppShell'
import { ToastProvider, useToast } from './state/ToastContext'
import { DashboardPage } from './pages/DashboardPage'
import { InventoryPage } from './pages/InventoryPage'
import { RoomBuilderPage } from './pages/RoomBuilderPage'
import { SystemPage } from './pages/SystemPage'
import { cloneConfig } from './lib/layout'
import { useBifrostData } from './state/useBifrostData'

function emptyConfig(): HassUiConfig {
  return {
    hidden_entity_ids: [],
    exclude_entity_ids: [],
    exclude_name_patterns: [],
    include_unavailable: true,
    rooms: [{ id: 'home-assistant', name: 'Home Assistant', source_area: null, auto_created: false }],
    entity_preferences: {},
    ignored_area_names: [],
    default_add_new_devices_to_hue: false,
    sync_hass_areas_to_rooms: true,
    fake_cloud_mode: 'off',
    fake_cloud_custom: {
      internet: false,
      signedon: false,
      incoming: false,
      outgoing: false,
      communication: 'disconnected',
      connectionstate: 'disconnected',
      legacy: false,
      trusted: true,
      action: 'none',
    },
    hass_timezone: null,
    hass_lat: null,
    hass_long: null,
  }
}

function viewFromHash(hash: string): ViewId {
  const value = hash.replace(/^#/, '').toLowerCase()
  if (value === 'rooms' || value === 'lights' || value === 'switches' || value === 'sensors' || value === 'hidden') return value === 'rooms' ? 'builder' : 'inventory'
  if (value === 'setup' || value === 'advanced' || value === 'bridge' || value === 'logs' || value === 'about') return 'system'
  return value === 'builder' || value === 'inventory' || value === 'system' ? value : 'overview'
}

function AppContent() {
  const data = useBifrostData()
  const toast = useToast()
  const [view, setView] = useState<ViewId>(() => viewFromHash(window.location.hash))
  const [draft, setDraft] = useState<HassUiConfig | null>(null)
  const [dirty, setDirty] = useState(false)
  const [saving, setSaving] = useState(false)

  const liveConfig = data.payload?.config || emptyConfig()
  const config = draft || liveConfig
  const entities = data.payload?.entities || []
  const runtimeConnected = !!data.runtime?.enabled && !!data.runtime.token_present

  useEffect(() => {
    const onHash = () => setView(viewFromHash(window.location.hash))
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  }, [])

  useEffect(() => {
    if (!dirty && data.payload?.config) {
      // The background poll is allowed to update the draft only while the user is not editing.
      setDraft(cloneConfig(data.payload.config))
    }
  }, [data.payload?.config, dirty])

  const counts = useMemo(() => ({
    rooms: config.rooms.length,
    entities: entities.length,
  }), [config.rooms.length, entities.length])

  function navigate(next: ViewId) {
    setView(next)
    window.location.hash = next
  }

  function changeDraft(next: HassUiConfig) {
    setDraft(cloneConfig(next))
    setDirty(true)
  }

  async function saveDraft() {
    if (!draft || saving) return
    setSaving(true)
    try {
      const saved = await putUiConfig(draft)
      await postSync()
      await postPatinaEvent('apply', 'room-builder-save').catch(() => {})
      setDraft(cloneConfig(saved))
      setDirty(false)
      data.refresh()
      toast.push('Room layout saved. Sync queued for Hue.', 'good')
    } catch (error) {
      toast.push(error instanceof Error ? error.message : String(error), 'bad')
    } finally {
      setSaving(false)
    }
  }

  function discardDraft() {
    setDraft(cloneConfig(liveConfig))
    setDirty(false)
    toast.push('Draft discarded.', 'neutral')
  }

  async function saveSystemConfig(next: HassUiConfig) {
    try {
      await putUiConfig(next)
      await postSync()
      data.refresh()
      toast.push('System preference saved. Sync queued.', 'good')
    } catch (error) {
      toast.push(error instanceof Error ? error.message : String(error), 'bad')
    }
  }

  function systemMessage(message: string, tone: 'good' | 'warn' | 'bad') {
    toast.push(message, tone)
  }

  return <AppShell view={view} onNavigate={navigate} runtimeConnected={runtimeConnected} entityCount={counts.entities} roomCount={counts.rooms} dirty={dirty} saving={saving} onSave={() => void saveDraft()} onDiscard={discardDraft} onRefresh={data.refresh} error={data.error}>
    {data.loading && !data.payload ? <div className="loading-state"><div className="loading-spinner" /><h2>Connecting to Bifrost</h2><p>Reading your Home Assistant bridge state…</p></div> : null}
    {!data.loading || data.payload ? <>
      {view === 'overview' ? <DashboardPage config={config} entities={entities} runtime={data.runtime} bridge={data.bridge} onNavigate={navigate} /> : null}
      {view === 'builder' ? <RoomBuilderPage config={config} entities={entities} onChange={changeDraft} /> : null}
      {view === 'inventory' ? <InventoryPage config={config} entities={entities} onChange={changeDraft} /> : null}
      {view === 'system' ? <SystemPage runtime={data.runtime} config={liveConfig} bridge={data.bridge} logs={data.payload?.logs || []} onSaveConfig={saveSystemConfig} onRefresh={data.refresh} onMessage={systemMessage} /> : null}
    </> : null}
  </AppShell>
}

export default function App() {
  return <ToastProvider><AppContent /></ToastProvider>
}
