import clsx from 'clsx'
import type React from 'react'
import { TactileButton } from './TactileButton'
import { usePatina } from '../state/PatinaContext'

export function ConfirmDialog(props: {
  open: boolean
  title: string
  body: React.ReactNode
  confirmText: string
  cancelText?: string
  tone?: 'neutral' | 'danger'
  onConfirm: () => void
  onClose: () => void
}) {
  const { patinaLevel } = usePatina()
  if (!props.open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/60" onClick={props.onClose} />
      <div
        className={clsx('plastic-panel noise-overlay relative w-full max-w-[520px] p-5')}
        style={{ ['--patina' as string]: String(patinaLevel) }}
      >
        <div className="text-sm font-semibold tracking-[0.08em] text-ink-1/70 uppercase">
          {props.title}
        </div>
        <div className="mt-2 text-sm text-ink-0">{props.body}</div>
        <div className="mt-5 flex justify-end gap-2">
          <TactileButton variant="neutral" onClick={props.onClose}>
            {props.cancelText ?? 'Cancel'}
          </TactileButton>
          <TactileButton
            variant={props.tone === 'danger' ? 'danger' : 'primary'}
            onClick={props.onConfirm}
          >
            {props.confirmText}
          </TactileButton>
        </div>
      </div>
    </div>
  )
}

