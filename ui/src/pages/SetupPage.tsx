import { useEffect, useState } from 'react'
import {
  connectRuntime,
  deleteToken,
  disconnectRuntime,
  postApply,
  postLinkButton,
  postPatinaEvent,
  postSync,
  putRuntimeConfig,
  putToken,
} from '../lib/api'
import type { HassRuntimeConfigPublic, HassUiConfig } from '../lib/types'
import { Panel } from '../components/Panel'
import { TactileButton } from '../components/TactileButton'
import { TextField } from '../components/TextField'
import { ToggleSwitch } from '../components/ToggleSwitch'

export function SetupPage(props: {
  runtime: HassRuntimeConfigPublic | null
  config: HassUiConfig
  onSaveConfig: (next: HassUiConfig) => Promise<void>
  onRefresh: () => void
}) {
  const [url, setUrl] = useState('')
  const [enabled, setEnabled] = useState(false)
  const [token, setToken] = useState('')
  const [busy, setBusy] = useState<string | null>(null)

  useEffect(() => {
    setUrl(props.runtime?.url || '')
    setEnabled(!!props.runtime?.enabled)
  }, [props.runtime?.url, props.runtime?.enabled])

  async function run(label: string, fn: () => Promise<void>) {
    setBusy(label)
    try {
      await fn()
    } finally {
      setBusy(null)
      props.onRefresh()
    }
  }

  return (
    <div className="space-y-4">
      <Panel
        title="Quick Start"
        subtitle="LAN-only setup. No YAML editing needed."
      >
        <ol className="list-decimal space-y-1 pl-5 text-[13px] text-ink-0">
          <li>Set Home Assistant URL and token.</li>
          <li>Connect, then run “Sync with Home Assistant”.</li>
          <li>Press bridge button and pair in Hue app.</li>
          <li>Use Lights/Switches/Sensors tabs to add devices.</li>
        </ol>
      </Panel>

      <Panel title="Home Assistant Connection" subtitle="Stored locally in Bifrost runtime state.">
        <div className="grid gap-3 sm:grid-cols-2">
          <TextField
            label="Home Assistant URL"
            value={url}
            onChange={setUrl}
            placeholder="http://homeassistant.local:8123"
            type="url"
            help="Example: http://192.168.2.5:8123"
          />
          <ToggleSwitch
            checked={enabled}
            onChange={setEnabled}
            label="Backend connected"
            help="Turns HA backend on/off. When off, no realtime updates are attempted."
            wearKey="setup:enabled"
          />
        </div>

        <div className="mt-2 grid gap-2 sm:grid-cols-2">
          <TextField
            label="Long-lived access token"
            value={token}
            onChange={setToken}
            placeholder="Paste token"
            type="password"
            help="Paste a Home Assistant long-lived access token."
          />
          <div className="flex items-end gap-1.5">
            <TactileButton
              variant="primary"
              disabled={!token.trim() || !!busy}
              onClick={() =>
                run('save-token', async () => {
                  await putToken(token.trim())
                  await postPatinaEvent('click', 'token-save').catch(() => {})
                  setToken('')
                })
              }
              wearKey="setup:token-save"
            >
              Save token
            </TactileButton>
            <TactileButton
              variant="danger"
              disabled={!!busy}
              onClick={() =>
                run('delete-token', async () => {
                  await deleteToken()
                  await postPatinaEvent('click', 'token-delete').catch(() => {})
                })
              }
              wearKey="setup:token-delete"
            >
              Delete token
            </TactileButton>
          </div>
        </div>

        <div className="mt-2 flex flex-wrap gap-1.5">
          <TactileButton
            variant="neutral"
            disabled={!!busy}
            onClick={() =>
              run('save-runtime', async () => {
                await putRuntimeConfig({ enabled, url: url.trim(), sync_mode: 'manual' })
                await postPatinaEvent('click', 'runtime-save').catch(() => {})
              })
            }
            wearKey="setup:runtime-save"
          >
            Save runtime
          </TactileButton>
          <TactileButton
            variant="good"
            disabled={!!busy}
            onClick={() => run('connect', () => connectRuntime())}
            wearKey="setup:connect"
          >
            Connect
          </TactileButton>
          <TactileButton
            variant="danger"
            disabled={!!busy}
            onClick={() => run('disconnect', () => disconnectRuntime())}
            wearKey="setup:disconnect"
          >
            Disconnect
          </TactileButton>
        </div>
      </Panel>

      <Panel title="Bridge Actions" subtitle="Applies to Bifrost/Hue mapping only.">
        <div className="flex flex-wrap gap-1.5">
          <TactileButton
            variant="primary"
            disabled={!!busy}
            onClick={() => run('sync', () => postSync())}
            wearKey="act:sync"
          >
            Sync with Home Assistant
          </TactileButton>
          <TactileButton
            variant="neutral"
            disabled={!!busy}
            onClick={() => run('button', () => postLinkButton())}
            wearKey="act:linkbutton"
          >
            Press bridge button
          </TactileButton>
          <TactileButton
            variant="neutral"
            disabled={!!busy}
            onClick={() => run('apply', () => postApply())}
            wearKey="act:apply"
          >
            Sync Hue app
          </TactileButton>
        </div>
      </Panel>

      <Panel title="Defaults" subtitle="Recommended defaults for large HA installs.">
        <div className="grid gap-2 sm:grid-cols-2">
          <ToggleSwitch
            checked={!!props.config.default_add_new_devices_to_hue}
            onChange={(v) =>
              props.onSaveConfig({ ...props.config, default_add_new_devices_to_hue: v })
            }
            label="Default add new devices to Hue"
            help="Recommended OFF. If ON, new entities are exposed by default."
            wearKey="cfg:default-add"
          />
          <ToggleSwitch
            checked={!!props.config.sync_hass_areas_to_rooms}
            onChange={(v) => props.onSaveConfig({ ...props.config, sync_hass_areas_to_rooms: v })}
            label="Sync HA areas to rooms"
            help="Creates rooms from Home Assistant areas on sync."
            wearKey="cfg:sync-areas"
          />
        </div>
      </Panel>
    </div>
  )
}
