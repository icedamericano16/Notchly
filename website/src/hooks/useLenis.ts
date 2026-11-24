import type Lenis from 'lenis'
import { useEffect, useSyncExternalStore } from 'react'
import { useHasInteracted } from './useFirstInteraction'
import { useReducedMotion } from './useReducedMotion'

/* One Lenis instance for the app, kept in a tiny external store so any component (and
   the GSAP scroll story later) can read it without prop drilling. */
let instance: Lenis | null = null
const listeners = new Set<() => void>()

function setInstance(next: Lenis | null): void {
  instance = next
  listeners.forEach((l) => l())
}

function subscribe(l: () => void): () => void {
  listeners.add(l)
  return () => listeners.delete(l)
}

/**
 * Mount once, at the app root. Lenis (and its per-frame loop) starts on the first
 * interaction, so it costs nothing while the page loads; it stays off with reduced motion.
 */
export function useLenisRoot(): void {
  const reduced = useReducedMotion()
  const interacted = useHasInteracted()
  useEffect(() => {
    if (reduced || !interacted) return
    let lenis: Lenis | null = null
    let cancelled = false
    void import('lenis').then(({ default: LenisCtor }) => {
      if (cancelled) return
      lenis = new LenisCtor({ autoRaf: true, anchors: { offset: -80 }, lerp: 0.12 })
      setInstance(lenis)
    })
    return () => {
      cancelled = true
      lenis?.destroy()
      setInstance(null)
    }
  }, [reduced, interacted])
}

/** The active Lenis instance, or null (reduced motion, or not mounted yet). */
export function useLenis(): Lenis | null {
  return useSyncExternalStore(
    subscribe,
    () => instance,
    () => null,
  )
}
