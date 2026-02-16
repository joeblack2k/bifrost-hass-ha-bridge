import { Panel } from '../components/Panel'
import { TactileButton } from '../components/TactileButton'

export function LogsPage(props: { logs: string[]; onRefresh: () => void }) {
  const lines = props.logs || []

  return (
    <div className="space-y-4">
      <Panel
        title="Logs"
        subtitle="Operational logs from the Home Assistant backend and bridge actions."
        right={
          <TactileButton variant="neutral" onClick={props.onRefresh} wearKey="logs:refresh">
            Refresh
          </TactileButton>
        }
      >
        <textarea
          readOnly
          value={lines.join('\n')}
          className="h-[60vh] w-full rounded-control border border-black/20 bg-[rgba(10,16,26,0.94)] p-3 font-mono text-[12px] text-[rgb(197,222,255)] shadow-inset"
        />
      </Panel>
    </div>
  )
}
