import { useEffect, useState } from 'react'
import type { IslandActivityId } from '@/content/island'

interface CycleOptions {
  intervalMs?: number
  paused?: boolean
}

/**
 * Steps through `sequence` every `intervalMs`. The timer restarts on every step (and on
 * manual selection), so a state never gets cut short after resuming.
 */
export function useIslandCycle(
  sequence: readonly IslandActivityId[],
  { intervalMs = 3800, paused = false }: CycleOptions = {},
) {
  const [index, setIndex] = useState(0)

  useEffect(() => {
    if (paused || sequence.length < 2) return
    const t = window.setTimeout(() => setIndex((i) => (i + 1) % sequence.length), intervalMs)
    return () => window.clearTimeout(t)
  }, [index, paused, intervalMs, sequence.length])

  const safeIndex = index % sequence.length
  return {
    activity: sequence[safeIndex] ?? 'idle',
    index: safeIndex,
    select: setIndex,
  }
}
