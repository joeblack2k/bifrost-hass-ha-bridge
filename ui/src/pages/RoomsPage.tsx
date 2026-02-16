import { useMemo, useState } from 'react'
import { deleteRoom, postPatinaEvent, postRoom, putRoomRename } from '../lib/api'
import type { HassRoomConfig, HassUiConfig } from '../lib/types'
import { Panel } from '../components/Panel'
import { TactileButton } from '../components/TactileButton'
import { TextField } from '../components/TextField'
import { ToggleSwitch } from '../components/ToggleSwitch'

export function RoomsPage(props: {
  config: HassUiConfig
  onSaveConfig: (next: HassUiConfig) => Promise<void>
  onRefresh: () => void
}) {
  const [newRoom, setNewRoom] = useState('')
  const [busy, setBusy] = useState<string | null>(null)

  const editable = useMemo(() => {
    return (props.config.rooms || []).slice().sort((a, b) => a.name.localeCompare(b.name))
  }, [props.config.rooms])

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
        title="Rooms"
        subtitle="Rooms are Hue rooms. You can auto-sync from Home Assistant areas, then rename freely."
      >
        <ToggleSwitch
          checked={!!props.config.sync_hass_areas_to_rooms}
          onChange={(v) =>
            props.onSaveConfig({ ...props.config, sync_hass_areas_to_rooms: v })
          }
          label="Sync HA areas to rooms"
          help="On manual sync, areas become rooms unless ignored."
          wearKey="rooms:sync-areas"
        />
      </Panel>

      <Panel title="Create Room" subtitle="Add a new Hue room for organizing entities.">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
          <div className="flex-1">
            <TextField
              label="Room name"
              value={newRoom}
              onChange={setNewRoom}
              placeholder="e.g. Cinema"
              help="Room IDs are derived automatically."
            />
          </div>
          <TactileButton
            variant="primary"
            disabled={!newRoom.trim() || !!busy}
            onClick={() =>
              run('create', async () => {
                await postRoom(newRoom.trim())
                await postPatinaEvent('click', 'room-create').catch(() => {})
                setNewRoom('')
              })
            }
            wearKey="rooms:create"
          >
            Add room
          </TactileButton>
        </div>
      </Panel>

      <Panel title="Existing Rooms" subtitle="Renames apply instantly to Hue. Removing reassigns to default room.">
        <div className="grid gap-3">
          {editable.map((r) => (
            <RoomRow
              key={r.id}
              room={r}
              disabled={!!busy}
              onRename={(name) =>
                run('rename', async () => {
                  await putRoomRename(r.id, name)
                  await postPatinaEvent('toggle', `room-rename:${r.id}`).catch(() => {})
                })
              }
              onDelete={() =>
                run('delete', async () => {
                  await deleteRoom(r.id)
                  await postPatinaEvent('reset', `room-delete:${r.id}`).catch(() => {})
                })
              }
            />
          ))}
        </div>
      </Panel>
    </div>
  )
}

function RoomRow(props: {
  room: HassRoomConfig
  disabled: boolean
  onRename: (name: string) => void
  onDelete: () => void
}) {
  const [name, setName] = useState(props.room.name)

  const isDefault = props.room.id === 'home-assistant'

  return (
    <div className="rounded-panel border border-black/10 bg-white/40 p-3 shadow-inset">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
        <div className="flex-1">
          <TextField
            label="Room"
            value={name}
            onChange={setName}
            help={
              isDefault
                ? 'Default room (cannot be removed).'
                : props.room.source_area
                  ? `Auto-created from HA area: ${props.room.source_area}`
                  : 'Custom room.'
            }
          />
          <div className="mt-1 font-mono text-[12px] text-ink-1/70">id: {props.room.id}</div>
        </div>
        <div className="flex gap-2">
          <TactileButton
            variant="neutral"
            disabled={props.disabled || name.trim() === props.room.name.trim()}
            onClick={() => props.onRename(name.trim())}
            wearKey={`room:save:${props.room.id}`}
          >
            Save
          </TactileButton>
          <TactileButton
            variant="danger"
            disabled={props.disabled || isDefault}
            onClick={props.onDelete}
            wearKey={`room:delete:${props.room.id}`}
          >
            Remove
          </TactileButton>
        </div>
      </div>
    </div>
  )
}
