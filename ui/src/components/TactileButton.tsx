import clsx from 'clsx'
import type React from 'react'
import { wearVars } from '../lib/patina'

type Variant = 'primary' | 'neutral' | 'good' | 'danger'

const variantClass: Record<Variant, string> = {
  neutral: 'text-ink-0',
  primary: 'text-white',
  good: 'text-white',
  danger: 'text-white',
}

export function TactileButton(props: {
  children: React.ReactNode
  onClick?: () => void
  disabled?: boolean
  variant?: Variant
  wearKey?: string
  className?: string
  title?: string
  type?: 'button' | 'submit'
}) {
  const variant = props.variant ?? 'neutral'

  const bg =
    variant === 'primary'
      ? 'bg-[linear-gradient(180deg,rgba(80,130,220,0.95),rgba(30,70,150,0.95))]'
      : variant === 'good'
        ? 'bg-[linear-gradient(180deg,rgba(90,170,125,0.95),rgba(55,120,85,0.95))]'
        : variant === 'danger'
          ? 'bg-[linear-gradient(180deg,rgba(220,95,98,0.95),rgba(150,55,60,0.95))]'
          : 'bg-[linear-gradient(180deg,rgba(66,91,138,0.9),rgba(34,53,90,0.96))]'

  const wearStyle = props.wearKey ? wearVars(props.wearKey) : undefined

  return (
    <button
      type={props.type ?? 'button'}
      title={props.title}
      disabled={props.disabled}
      onClick={props.onClick}
      className={clsx(
        'tactile-control fingerprint select-none px-3 py-1.5 text-xs font-semibold tracking-[0.04em]',
        'active:translate-y-[1px] transition-transform',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        bg,
        variantClass[variant],
        props.className,
      )}
      style={wearStyle}
    >
      {props.children}
    </button>
  )
}
