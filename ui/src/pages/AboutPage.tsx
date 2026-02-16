import { useEffect, useMemo, useState } from 'react'
import type { HassBridgeInfo, HassPatinaPublic } from '../lib/types'
import { Panel } from '../components/Panel'
import { ToggleSwitch } from '../components/ToggleSwitch'
import { usePatina } from '../state/PatinaContext'

export function AboutPage(props: {
  bridge: HassBridgeInfo | null
  patina?: HassPatinaPublic
}) {
  const { patinaLevel, actualLevel, stage, setPreviewLevel } = usePatina()
  const [preview, setPreview] = useState(false)
  const [slider, setSlider] = useState(0)

  useEffect(() => {
    setSlider(actualLevel)
  }, [actualLevel])

  useEffect(() => {
    if (preview) {
      setPreviewLevel(slider)
    } else {
      setPreviewLevel(null)
    }
    return () => setPreviewLevel(null)
  }, [preview, slider, setPreviewLevel])

  const stageLabel = useMemo(() => {
    if (stage === 'loved') return 'Loved'
    if (stage === 'used') return 'Used'
    return 'Fresh'
  }, [stage])

  return (
    <div className="space-y-4">
      <Panel title="About" subtitle="Bifrost Home Assistant bridge UI">
        <div className="grid gap-2 sm:grid-cols-2">
          <AboutKv k="Bridge" v={props.bridge?.bridge_name || '-'} />
          <AboutKv k="Software" v={props.bridge?.software_version || '-'} />
          <AboutKv k="Bridge ID" v={props.bridge?.bridge_id || '-'} mono />
          <AboutKv k="IP Address" v={props.bridge?.ipaddress || '-'} mono />
        </div>
      </Panel>

      <Panel title="Digital Patina" subtitle="Interface wear based on usage and install age.">
        <div className="grid gap-2 sm:grid-cols-2">
          <AboutKv k="Install date" v={props.patina?.install_date || '-'} mono />
          <AboutKv k="Interactions" v={String(props.patina?.interaction_count ?? 0)} mono />
          <AboutKv k="Computed level" v={`${actualLevel}/100`} />
          <AboutKv k="Stage" v={stageLabel} />
        </div>

        <div className="mt-4 rounded-panel border border-black/10 bg-white/35 p-3">
          <ToggleSwitch
            checked={preview}
            onChange={setPreview}
            label="Patina preview mode"
            help="Debug visual aging without changing stored data."
            wearKey="about:preview-toggle"
          />
          <div className="mt-3">
            <input
              type="range"
              min={0}
              max={100}
              value={slider}
              onChange={(e) => setSlider(Number(e.target.value))}
              disabled={!preview}
              className="w-full accent-[rgb(50,120,210)] disabled:opacity-40"
            />
            <div className="mt-1 text-xs text-ink-1/70">
              Active UI patina: <span className="font-semibold">{patinaLevel}</span>
            </div>
          </div>
        </div>
      </Panel>

      <Panel
        title="What This UI Does"
        subtitle="English-only v1, LAN-only configuration panel, no file editing needed."
      >
        <ul className="list-disc space-y-1 pl-5 text-sm text-ink-0">
          <li>Connect/disconnect Home Assistant and manage token/runtime settings.</li>
          <li>Expose lights, switches, and sensors to the Hue app instantly.</li>
          <li>Map entities to rooms, create/remove rooms, and sync areas.</li>
          <li>Show bridge diagnostics and trigger bridge actions safely.</li>
        </ul>
      </Panel>
    </div>
  )
}

function AboutKv(props: { k: string; v: string; mono?: boolean }) {
  return (
    <div className="rounded-control border border-black/10 bg-white/45 px-3 py-2 shadow-inset">
      <div className="text-[11px] font-semibold tracking-[0.08em] text-ink-1/70 uppercase">{props.k}</div>
      <div className={`mt-1 break-words text-[14px] text-ink-0 ${props.mono ? 'font-mono' : 'font-semibold'}`}>
        {props.v}
      </div>
    </div>
  )
}
