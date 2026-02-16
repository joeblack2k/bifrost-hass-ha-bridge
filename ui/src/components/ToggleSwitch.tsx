import clsx from 'clsx'
import type React from 'react'
import { wearVars } from '../lib/patina'

export function ToggleSwitch(props: {
  checked: boolean
  onChange: (next: boolean) => void
  label?: string
  help?: React.ReactNode
  disabled?: boolean
  wearKey?: string
}) {
  const wearStyle = props.wearKey ? wearVars(props.wearKey) : undefined

  return (
    <div className="flex items-center justify-between gap-3">
      <div className="min-w-0">
        {props.label && <div className="text-xs font-semibold text-ink-0">{props.label}</div>}
        {props.help && <div className="mt-1 text-[11px] text-ink-1">{props.help}</div>}
      </div>
      <button
        type="button"
        aria-pressed={props.checked}
        disabled={props.disabled}
        onClick={() => props.onChange(!props.checked)}
        className={clsx(
          'fingerprint relative h-6 w-10 rounded-full border transition',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          props.checked
            ? 'border-transparent bg-[linear-gradient(180deg,rgba(70,125,215,1),rgba(30,75,165,1))]'
            : 'border-black/20 bg-[linear-gradient(180deg,rgba(167,183,214,0.42),rgba(68,88,130,0.78))]',
        )}
        style={wearStyle}
      >
        <span
          className={clsx(
            'absolute top-[2px] h-5 w-5 rounded-full',
            'bg-[linear-gradient(180deg,rgba(249,252,255,0.95),rgba(219,228,244,1))]',
            'shadow-[inset_0_1px_0_rgba(255,255,255,0.9),0_5px_10px_rgba(0,0,0,0.30)]',
            props.checked ? 'left-[18px]' : 'left-[2px]',
            'transition-[left] duration-150 ease-out',
          )}
        />
      </button>
    </div>
  )
}
