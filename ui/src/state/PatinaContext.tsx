import React, { createContext, useContext, useMemo, useState } from 'react'
import type { HassPatinaPublic } from '../lib/types'

type PatinaCtx = {
  patinaLevel: number
  stage: HassPatinaPublic['stage']
  actualLevel: number
  setPreviewLevel: (level: number | null) => void
}

const Ctx = createContext<PatinaCtx>({
  patinaLevel: 0,
  stage: 'fresh',
  actualLevel: 0,
  setPreviewLevel: () => {},
})

export function PatinaProvider(props: { patina?: HassPatinaPublic; children: React.ReactNode }) {
  const [previewLevel, setPreviewLevel] = useState<number | null>(null)

  const value = useMemo(() => {
    const actualLevel = Math.max(0, Math.min(100, props.patina?.patina_level ?? 0))
    const level = previewLevel === null ? actualLevel : Math.max(0, Math.min(100, previewLevel))
    const stage = props.patina?.stage ?? 'fresh'
    return {
      patinaLevel: level,
      stage,
      actualLevel,
      setPreviewLevel,
    }
  }, [props.patina, previewLevel])

  return <Ctx.Provider value={value}>{props.children}</Ctx.Provider>
}

export function usePatina() {
  return useContext(Ctx)
}
