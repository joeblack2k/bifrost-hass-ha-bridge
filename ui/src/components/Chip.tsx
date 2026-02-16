import clsx from 'clsx'
import type React from 'react'

export function Chip(props: {
  children: React.ReactNode
  tone?: 'neutral' | 'good' | 'warn' | 'bad'
}) {
  const tone = props.tone ?? 'neutral'
  const cls =
    tone === 'good'
      ? 'bg-[rgba(76,165,118,0.18)] text-ink-0 border-[rgba(76,165,118,0.40)]'
      : tone === 'warn'
        ? 'bg-[rgba(220,120,42,0.20)] text-ink-0 border-[rgba(220,120,42,0.40)]'
        : tone === 'bad'
          ? 'bg-[rgba(205,72,74,0.2)] text-ink-0 border-[rgba(205,72,74,0.40)]'
          : 'bg-[rgba(137,160,212,0.16)] text-ink-0 border-[rgba(123,145,198,0.40)]'

  return (
    <span
      className={clsx(
        'inline-flex items-center rounded-full border px-2 py-[1px] text-[11px] font-semibold',
        cls,
      )}
    >
      {props.children}
    </span>
  )
}
