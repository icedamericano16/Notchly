import { useSyncExternalStore } from 'react'

/*
 * One shared one-second clock for every demo readout (timer, song position, download), so
 * the compact and expanded views of a state always agree and only one interval runs.
 */
const start = Date.now()
let seconds = 0
let timer: number | undefined
const listeners = new Set<() => void>()

function subscribe(l: () => void): () => void {
  listeners.add(l)
  if (timer === undefined) {
    timer = window.setInterval(() => {
      seconds = Math.floor((Date.now() - start) / 1000)
      listeners.forEach((fn) => fn())
    }, 1000)
  }
  return () => {
    listeners.delete(l)
    if (listeners.size === 0 && timer !== undefined) {
      window.clearInterval(timer)
      timer = undefined
    }
  }
}

/** Whole seconds since the page loaded; ticks once a second while anything listens. */
export function useDemoClock(): number {
  return useSyncExternalStore(
    subscribe,
    () => seconds,
    () => 0,
  )
}

/** "4:05" */
export function formatClock(totalSec: number): string {
  const s = Math.max(0, Math.floor(totalSec))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}
