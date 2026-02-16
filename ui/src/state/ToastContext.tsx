import React, { createContext, useContext, useMemo, useState } from 'react'

type ToastTone = 'neutral' | 'good' | 'warn' | 'bad'
export type Toast = { id: string; message: string; tone: ToastTone }

type ToastCtx = {
  push: (message: string, tone?: ToastTone) => void
}

const Ctx = createContext<ToastCtx>({ push: () => {} })

function uid() {
  return Math.random().toString(16).slice(2) + Date.now().toString(16)
}

export function ToastProvider(props: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])

  const api = useMemo<ToastCtx>(() => {
    return {
      push: (message, tone = 'neutral') => {
        const id = uid()
        setToasts((t) => [{ id, message, tone }, ...t].slice(0, 4))
        window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 2500)
      },
    }
  }, [])

  return (
    <Ctx.Provider value={api}>
      {props.children}
      <div className="fixed bottom-3 left-3 right-3 z-50 flex flex-col gap-2 sm:left-auto sm:right-4 sm:w-[420px]">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={[
              'plastic-panel noise-overlay px-4 py-3 text-sm font-semibold',
              t.tone === 'good'
                ? 'border-[rgba(76,165,118,0.65)]'
                : t.tone === 'warn'
                  ? 'border-[rgba(220,120,42,0.65)]'
                  : t.tone === 'bad'
                    ? 'border-[rgba(205,72,74,0.65)]'
                    : '',
            ].join(' ')}
          >
            {t.message}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  )
}

export function useToast() {
  return useContext(Ctx)
}

