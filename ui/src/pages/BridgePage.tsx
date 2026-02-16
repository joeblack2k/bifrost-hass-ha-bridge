import { useMemo, useState } from 'react'
import {
  postApply,
  postLinkButton,
  postPatinaEvent,
  postResetBridge,
  postSync,
} from '../lib/api'
import type { HassBridgeInfo, HassUiPayload } from '../lib/types'
import { ConfirmDialog } from '../components/ConfirmDialog'
import { Panel } from '../components/Panel'
import { TactileButton } from '../components/TactileButton'

export function BridgePage(props: {
  payload: HassUiPayload
  bridge: HassBridgeInfo | null
  onRefresh: () => void
}) {
  const [busy, setBusy] = useState<string | null>(null)
  const [confirmReset, setConfirmReset] = useState(false)

  const kv = useMemo(() => {
    const b = props.bridge
    if (!b) return []
    return [
      ['Bridge', b.bridge_name],
      ['Bridge ID', b.bridge_id],
      ['Software', b.software_version],
      ['IP', b.ipaddress],
      ['MAC', b.mac],
      ['Timezone', b.timezone],
      ['Entities', `${b.total_entities} total / ${b.included_entities} added / ${b.hidden_entities} hidden`],
      ['Rooms', String(b.room_count)],
      ['Last sync', b.sync_status?.last_sync_at || 'never'],
      ['Sync result', b.sync_status?.last_sync_result || '-'],
      ['Sync ms', String(b.sync_status?.last_sync_duration_ms ?? '-')],
      ['Link button', b.linkbutton_active ? 'active' : 'inactive'],
    ] as const
  }, [props.bridge])

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
      <Panel title="Bridge" subtitle="Diagnostics and bridge actions.">
        <div className="grid gap-2 sm:grid-cols-2">
          {kv.map(([k, v]) => (
            <div key={k} className="rounded-control border border-black/10 bg-white/45 px-3 py-2 shadow-inset">
              <div className="text-[11px] font-semibold tracking-[0.08em] text-ink-1/70 uppercase">{k}</div>
              <div className="mt-1 break-words text-[14px] font-semibold text-ink-0">{v}</div>
            </div>
          ))}
        </div>

        <div className="mt-4 flex flex-wrap gap-2">
          <TactileButton
            variant="primary"
            disabled={!!busy}
            onClick={() => run('sync', () => postSync())}
            wearKey="bridge:sync"
          >
            Sync with Home Assistant
          </TactileButton>
          <TactileButton
            variant="neutral"
            disabled={!!busy}
            onClick={() => run('apply', () => postApply())}
            wearKey="bridge:apply"
          >
            Sync Hue app
          </TactileButton>
          <TactileButton
            variant="neutral"
            disabled={!!busy}
            onClick={() => run('button', () => postLinkButton())}
            wearKey="bridge:button"
          >
            Press bridge button
          </TactileButton>
          <TactileButton
            variant="danger"
            disabled={!!busy}
            onClick={() => setConfirmReset(true)}
            wearKey="bridge:reset"
          >
            Reset Hue bridge
          </TactileButton>
        </div>
      </Panel>

      <Panel title="Sync Status" subtitle="This is about importing HA entities and areas.">
        <div className="text-sm text-ink-0">
          Sync in progress:{' '}
          <span className="font-semibold">
            {props.payload.sync.sync_in_progress ? 'yes' : 'no'}
          </span>
        </div>
        <div className="mt-1 text-sm text-ink-0">
          Last sync: <span className="font-mono">{props.payload.sync.last_sync_at || 'never'}</span>
        </div>
        <div className="mt-1 text-sm text-ink-0">
          Result: <span className="font-mono">{props.payload.sync.last_sync_result || '-'}</span>
        </div>
      </Panel>

      <ConfirmDialog
        open={confirmReset}
        title="Reset Hue bridge?"
        tone="danger"
        confirmText="Reset"
        body={
          <div className="space-y-2">
            <div>This clears the Hue resource database inside Bifrost.</div>
            <div className="font-semibold">You will need to re-pair the bridge in the Hue app.</div>
          </div>
        }
        onClose={() => setConfirmReset(false)}
        onConfirm={() =>
          run('reset', async () => {
            setConfirmReset(false)
            await postResetBridge()
            await postPatinaEvent('reset', 'bridge-reset').catch(() => {})
          })
        }
      />
    </div>
  )
}

