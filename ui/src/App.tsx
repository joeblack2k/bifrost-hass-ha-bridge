import { useEffect, useMemo, useRef, useState } from 'react'
import clsx from 'clsx'
import { patchEntity, postPatinaEvent, putUiConfig } from './lib/api'
import type { HassEntitySummary, HassSensorKind, HassUiConfig } from './lib/types'
import { Panel } from './components/Panel'
import { TactileButton } from './components/TactileButton'
import { AboutPage } from './pages/AboutPage'
import { BridgePage } from './pages/BridgePage'
import { EntitiesPage } from './pages/EntitiesPage'
import { LogsPage } from './pages/LogsPage'
import { RoomsPage } from './pages/RoomsPage'
import { SetupPage } from './pages/SetupPage'
import { PatinaProvider } from './state/PatinaContext'
import { ToastProvider, useToast } from './state/ToastContext'
import { useBifrostData } from './state/useBifrostData'

type TabId =
  | 'setup'
  | 'lights'
  | 'switches'
  | 'sensors'
  | 'hidden'
  | 'rooms'
  | 'bridge'
  | 'logs'
  | 'about'

const TABS: Array<{ id: TabId; label: string }> = [
  { id: 'setup', label: 'Setup' },
  { id: 'lights', label: 'Lights' },
  { id: 'switches', label: 'Switches' },
  { id: 'sensors', label: 'Sensors' },
  { id: 'hidden', label: 'Hidden' },
  { id: 'rooms', label: 'Rooms' },
  { id: 'bridge', label: 'Bridge' },
  { id: 'logs', label: 'Logs' },
  { id: 'about', label: 'About' },
]

function tabFromHash(hash: string): TabId {
  const id = hash.replace(/^#/, '').toLowerCase()
  return (TABS.find((t) => t.id === id)?.id || 'setup') as TabId
}

function emptyConfig(): HassUiConfig {
  return {
    hidden_entity_ids: [],
    exclude_entity_ids: [],
    exclude_name_patterns: [],
    include_unavailable: true,
    rooms: [],
    entity_preferences: {},
    ignored_area_names: [],
    default_add_new_devices_to_hue: false,
    sync_hass_areas_to_rooms: true,
  }
}

function AppContent(props: { data: ReturnType<typeof useBifrostData> }) {
  const data = props.data
  const toast = useToast()

  const [tab, setTab] = useState<TabId>(() => tabFromHash(window.location.hash))
  const aliasDebounce = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    const onHash = () => setTab(tabFromHash(window.location.hash))
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  }, [])

  useEffect(() => {
    const timers = aliasDebounce.current
    return () => {
      for (const timeout of timers.values()) {
        window.clearTimeout(timeout)
      }
      timers.clear()
    }
  }, [])

  const payload = data.payload
  const config = payload?.config || emptyConfig()
  const entities = useMemo(() => payload?.entities || [], [payload?.entities])
  const rooms = config.rooms || []

  const counters = useMemo(() => {
    const lights = entities.filter((e) => e.domain === 'light').length
    const switches = entities.filter((e) => e.domain === 'switch').length
    const sensors = entities.filter((e) => e.domain === 'binary_sensor').length
    const hidden = entities.filter((e) => !e.included).length
    return { lights, switches, sensors, hidden }
  }, [entities])

  async function callWithToast(okText: string, fn: () => Promise<void>) {
    try {
      await fn()
      data.refresh()
      toast.push(okText, 'good')
    } catch (err) {
      toast.push(err instanceof Error ? err.message : String(err), 'bad')
    }
  }

  async function saveConfig(next: HassUiConfig) {
    await callWithToast('Configuration saved', async () => {
      await putUiConfig(next)
      await postPatinaEvent('apply', 'save-config').catch(() => {})
    })
  }

  function setIncluded(entity: HassEntitySummary, included: boolean) {
    void callWithToast(`${included ? 'Added to Hue' : 'Hidden from Hue'}: ${entity.entity_id}`, async () => {
      await patchEntity(entity.entity_id, { hidden: !included })
      await postPatinaEvent('toggle', `included:${entity.entity_id}`).catch(() => {})
    })
  }

  function setRoom(entity: HassEntitySummary, roomId: string) {
    void callWithToast(`Room updated: ${entity.entity_id}`, async () => {
      await patchEntity(entity.entity_id, { room_id: roomId })
      await postPatinaEvent('toggle', `room:${entity.entity_id}`).catch(() => {})
    })
  }

  function setAlias(entity: HassEntitySummary, alias: string) {
    const key = entity.entity_id
    const existing = aliasDebounce.current.get(key)
    if (existing) {
      window.clearTimeout(existing)
    }
    const timeout = window.setTimeout(() => {
      aliasDebounce.current.delete(key)
      void callWithToast(`Alias saved: ${entity.entity_id}`, async () => {
        await patchEntity(entity.entity_id, { alias })
        await postPatinaEvent('click', `alias:${entity.entity_id}`).catch(() => {})
      })
    }, 350)
    aliasDebounce.current.set(key, timeout)
  }

  function setSensorKind(entity: HassEntitySummary, kind: HassSensorKind) {
    void callWithToast(`Sensor type updated: ${entity.entity_id}`, async () => {
      await patchEntity(entity.entity_id, { sensor_kind: kind })
      await postPatinaEvent('toggle', `sensor-kind:${entity.entity_id}`).catch(() => {})
    })
  }

  function setSensorEnabled(entity: HassEntitySummary, enabled: boolean) {
    void callWithToast(`Sensor ${enabled ? 'enabled' : 'disabled'}: ${entity.entity_id}`, async () => {
      await patchEntity(entity.entity_id, { enabled })
      await postPatinaEvent('toggle', `sensor-enabled:${entity.entity_id}`).catch(() => {})
    })
  }

  return (
    <div className="mx-auto max-w-[1400px] px-2 pb-10 pt-2 sm:px-4 sm:pt-4">
      <header className="sticky top-2 z-20 mb-3">
        <Panel
          title="Bifrost HA Bridge"
          subtitle="Direct control for what appears in the Hue app."
          right={
            <TactileButton variant="neutral" onClick={data.refresh} wearKey="app:refresh">
              Refresh
            </TactileButton>
          }
        >
          <div className="mt-2 flex flex-wrap gap-1.5">
            {TABS.map((t) => (
              <button
                key={t.id}
                type="button"
                onClick={() => {
                  setTab(t.id)
                  window.location.hash = t.id
                }}
                className={clsx(
                  'tactile-control fingerprint rounded-full px-2.5 py-1 text-[11px] font-semibold tracking-[0.06em] uppercase',
                  tab === t.id
                    ? 'bg-[linear-gradient(180deg,rgba(90,135,220,0.95),rgba(35,75,165,0.96))] text-white'
                    : 'text-ink-0',
                )}
              >
                {t.label}
                {t.id === 'lights' && counters.lights > 0 ? ` (${counters.lights})` : ''}
                {t.id === 'switches' && counters.switches > 0 ? ` (${counters.switches})` : ''}
                {t.id === 'sensors' && counters.sensors > 0 ? ` (${counters.sensors})` : ''}
                {t.id === 'hidden' && counters.hidden > 0 ? ` (${counters.hidden})` : ''}
              </button>
            ))}
          </div>

          {data.error ? (
            <div className="mt-3 rounded-control border border-[rgba(205,72,74,0.55)] bg-[rgba(205,72,74,0.15)] px-3 py-2 text-sm text-ink-0">
              API error: {data.error}
            </div>
          ) : null}
        </Panel>
      </header>

      {data.loading && !payload ? (
        <Panel title="Loading" subtitle="Fetching bridge state...">
          <div className="text-sm text-ink-1/70">Please wait.</div>
        </Panel>
      ) : (
        <main>
          {tab === 'setup' && (
            <SetupPage
              runtime={data.runtime}
              config={config}
              onSaveConfig={saveConfig}
              onRefresh={data.refresh}
            />
          )}

          {tab === 'lights' && (
            <EntitiesPage
              title="Lights"
              subtitle="Home Assistant light entities exposed as Hue lights."
              entities={entities}
              rooms={rooms}
              predicate={(e) => e.domain === 'light'}
              onSetIncluded={setIncluded}
              onSetRoom={setRoom}
              onSetAlias={setAlias}
              onSetSensorKind={setSensorKind}
              onSetSensorEnabled={setSensorEnabled}
            />
          )}

          {tab === 'switches' && (
            <EntitiesPage
              title="Switches"
              subtitle="Home Assistant switch entities exposed as Hue plugs."
              entities={entities}
              rooms={rooms}
              predicate={(e) => e.domain === 'switch'}
              onSetIncluded={setIncluded}
              onSetRoom={setRoom}
              onSetAlias={setAlias}
              onSetSensorKind={setSensorKind}
              onSetSensorEnabled={setSensorEnabled}
            />
          )}

          {tab === 'sensors' && (
            <EntitiesPage
              title="Sensors"
              subtitle="Binary sensors mapped as Hue motion/contact sensors."
              entities={entities}
              rooms={rooms}
              predicate={(e) => e.domain === 'binary_sensor'}
              onSetIncluded={setIncluded}
              onSetRoom={setRoom}
              onSetAlias={setAlias}
              onSetSensorKind={setSensorKind}
              onSetSensorEnabled={setSensorEnabled}
            />
          )}

          {tab === 'hidden' && (
            <EntitiesPage
              title="Hidden"
              subtitle="All entities currently not exposed in Hue."
              entities={entities}
              rooms={rooms}
              predicate={(e) => !e.included}
              onSetIncluded={setIncluded}
              onSetRoom={setRoom}
              onSetAlias={setAlias}
              onSetSensorKind={setSensorKind}
              onSetSensorEnabled={setSensorEnabled}
            />
          )}

          {tab === 'rooms' && (
            <RoomsPage config={config} onSaveConfig={saveConfig} onRefresh={data.refresh} />
          )}

          {tab === 'bridge' && payload && (
            <BridgePage payload={payload} bridge={data.bridge} onRefresh={data.refresh} />
          )}

          {tab === 'logs' && <LogsPage logs={payload?.logs || []} onRefresh={data.refresh} />}

          {tab === 'about' && <AboutPage bridge={data.bridge} patina={payload?.patina} />}
        </main>
      )}
    </div>
  )
}

export default function App() {
  const data = useBifrostData()
  return (
    <ToastProvider>
      <PatinaProvider patina={data.payload?.patina}>
        <AppContent data={data} />
      </PatinaProvider>
    </ToastProvider>
  )
}
