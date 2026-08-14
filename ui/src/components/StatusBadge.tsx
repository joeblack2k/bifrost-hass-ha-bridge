import clsx from 'clsx'
import type { ReactNode } from 'react'
import { Icon } from './Icon'

export function StatusBadge(props: {
  tone?: 'neutral' | 'good' | 'warn' | 'bad'
  children: ReactNode
  dot?: boolean
}) {
  const tone = props.tone ?? 'neutral'
  return (
    <span className={clsx('status-badge', `status-badge-${tone}`)}>
      {props.dot ? <Icon name="circle" size={8} /> : null}
      {props.children}
    </span>
  )
}
