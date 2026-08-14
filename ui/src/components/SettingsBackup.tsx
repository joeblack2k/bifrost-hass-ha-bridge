import { useRef, useState } from 'react'
import { getBridgeSettingsExport, importBridgeSettings } from '../lib/api'
import { Icon } from './Icon'

export function SettingsBackup(props: { onRefresh: () => void; onMessage: (message: string, tone: 'good' | 'warn' | 'bad') => void }) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [busy, setBusy] = useState(false)

  async function exportSettings() {
    setBusy(true)
    try {
      const settings = await getBridgeSettingsExport()
      const blob = new Blob([`${JSON.stringify(settings, null, 2)}\n`], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const anchor = document.createElement('a')
      anchor.href = url
      anchor.download = 'bifrost-bridge-settings.json'
      anchor.click()
      URL.revokeObjectURL(url)
      props.onMessage('Bridge settings exported.', 'good')
    } catch (error) {
      props.onMessage(error instanceof Error ? error.message : String(error), 'bad')
    } finally {
      setBusy(false)
    }
  }

  async function importFile(file: File) {
    if (!window.confirm('Import bridge settings? This replaces the current rooms and lamp assignments.')) {
      if (inputRef.current) inputRef.current.value = ''
      return
    }
    setBusy(true)
    try {
      const settings = JSON.parse(await file.text()) as unknown
      await importBridgeSettings(settings)
      props.onRefresh()
      props.onMessage('Bridge settings imported and sync queued.', 'good')
    } catch (error) {
      props.onMessage(error instanceof Error ? error.message : String(error), 'bad')
    } finally {
      setBusy(false)
      if (inputRef.current) inputRef.current.value = ''
    }
  }

  return <section className="system-card"><div className="panel-heading"><div><div className="eyebrow">Backup</div><h2>Bridge settings</h2></div><span className="panel-heading-hint">Local only</span></div><p className="system-card-copy">Export your rooms, lamp assignments and preferences to a JSON backup. Importing replaces only these bridge settings; the Home Assistant token is never included.</p><div className="system-actions"><button className="button button-secondary button-small" type="button" disabled={busy} onClick={() => void exportSettings()}><Icon name="download" size={15} /> Export settings</button><label className="button button-ghost button-small"><Icon name="upload" size={15} /> Import settings<input ref={inputRef} type="file" accept="application/json,.json" disabled={busy} onChange={(event) => { const file = event.target.files?.[0]; if (file) void importFile(file) }} style={{ display: 'none' }} /></label></div></section>
}
