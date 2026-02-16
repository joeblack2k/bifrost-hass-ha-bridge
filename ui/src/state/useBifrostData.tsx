import { useEffect, useMemo, useState } from 'react'
import { getBridgeInfo, getRuntimeConfig, getUiPayload } from '../lib/api'
import type { HassBridgeInfo, HassRuntimeConfigPublic, HassUiPayload } from '../lib/types'

export function useBifrostData() {
  const [payload, setPayload] = useState<HassUiPayload | null>(null)
  const [bridge, setBridge] = useState<HassBridgeInfo | null>(null)
  const [runtime, setRuntime] = useState<HassRuntimeConfigPublic | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const [refreshNonce, setRefreshNonce] = useState(0)

  const visible = usePageVisible()
  const intervalMs = visible ? 2000 : 10000

  useEffect(() => {
    let alive = true

    async function tick() {
      try {
        const [p, b, r] = await Promise.all([
          getUiPayload(),
          getBridgeInfo(),
          getRuntimeConfig(),
        ])
        if (!alive) return
        setPayload(p)
        setBridge(b)
        setRuntime(r)
        setError(null)
      } catch (e) {
        if (!alive) return
        setError(e instanceof Error ? e.message : String(e))
      } finally {
        if (alive) setLoading(false)
      }
    }

    tick()
    const id = window.setInterval(() => {
      // allow callers to request an immediate refresh without changing deps
      if (!alive) return
      void tick()
    }, intervalMs)

    return () => {
      alive = false
      window.clearInterval(id)
    }
  }, [intervalMs, refreshNonce])

  const api = useMemo(() => {
    return {
      payload,
      bridge,
      runtime,
      error,
      loading,
      refresh: () => {
        setRefreshNonce((x) => x + 1)
        setLoading(true)
      },
      setPayload,
      setBridge,
      setRuntime,
    }
  }, [payload, bridge, runtime, error, loading])

  return api
}

function usePageVisible(): boolean {
  const [v, setV] = useState(() => document.visibilityState === 'visible')
  useEffect(() => {
    const on = () => setV(document.visibilityState === 'visible')
    document.addEventListener('visibilitychange', on)
    return () => document.removeEventListener('visibilitychange', on)
  }, [])
  return v
}
