export type PatinaStage = 'fresh' | 'used' | 'loved'

export function stageFromLevel(level: number): PatinaStage {
  if (level >= 71) return 'loved'
  if (level >= 26) return 'used'
  return 'fresh'
}

// Deterministic hash for wear placement. Keep fast and stable.
export function hashString(input: string): number {
  let h = 2166136261
  for (let i = 0; i < input.length; i++) {
    h ^= input.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

export function wearVars(seed: string): Record<string, string> {
  const h = hashString(seed)
  const x = 30 + (h % 55) // 30..84
  const y = 22 + ((h >>> 8) % 60) // 22..81
  return {
    // used by .fingerprint::after
    ['--wear-x' as string]: `${x}%`,
    ['--wear-y' as string]: `${y}%`,
  }
}

