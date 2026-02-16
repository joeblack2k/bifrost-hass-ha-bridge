import clsx from 'clsx'
import type React from 'react'

export function TextField(props: {
  label: string
  value: string
  onChange: (v: string) => void
  placeholder?: string
  type?: 'text' | 'password' | 'url'
  help?: React.ReactNode
  right?: React.ReactNode
  className?: string
}) {
  return (
    <label className={clsx('block', props.className)}>
      <div className="flex items-end justify-between gap-3">
        <div className="text-[12px] font-semibold tracking-[0.08em] text-ink-1/70 uppercase">
          {props.label}
        </div>
        {props.right}
      </div>
      {props.help && <div className="mt-1 text-xs text-ink-1/70">{props.help}</div>}
      <input
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        placeholder={props.placeholder}
        type={props.type ?? 'text'}
        className={clsx(
          'mt-2 w-full rounded-control border border-black/25',
          'bg-[linear-gradient(180deg,rgba(23,36,58,0.96),rgba(16,27,44,0.97))]',
          'px-2.5 py-1.5 text-[13px] text-ink-0 shadow-inset',
          'focus:outline-none focus:ring-2 focus:ring-accent-blue/60',
        )}
      />
    </label>
  )
}
