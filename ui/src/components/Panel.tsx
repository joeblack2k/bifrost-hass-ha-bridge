import clsx from 'clsx'
import type React from 'react'
import { usePatina } from '../state/PatinaContext'

export function Panel(props: {
  title?: string
  subtitle?: string
  right?: React.ReactNode
  className?: string
  children: React.ReactNode
}) {
  const { patinaLevel } = usePatina()
  return (
    <section
      className={clsx('plastic-panel noise-overlay p-4 sm:p-5', props.className)}
      style={{ ['--patina' as string]: String(patinaLevel) }}
    >
      {(props.title || props.right) && (
        <header className="mb-3 flex items-start justify-between gap-3">
          <div>
            {props.title && (
              <h2 className="text-[13px] font-semibold tracking-[0.08em] text-ink-1/70 uppercase">
                {props.title}
              </h2>
            )}
            {props.subtitle && (
              <p className="mt-1 text-sm text-ink-1/70">{props.subtitle}</p>
            )}
          </div>
          {props.right}
        </header>
      )}
      {props.children}
    </section>
  )
}
