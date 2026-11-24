import { useEffect, useSyncExternalStore } from 'react'

const EVENTS = ['pointermove', 'pointerdown', 'wheel', 'touchstart', 'keydown', 'scroll'] as const

let interacted = false
const listeners = new Set<() => void>()

function markInteracted() {
  if (interacted) return
  interacted = true
  document.documentElement.dataset.interacted = ''
  EVENTS.forEach((e) => window.removeEventListener(e, markInteracted))
  listeners.forEach((l) => l())
}

/**
 * Mount once at the root. Decorative loops (aurora, beams, marquee, bento visuals; see
 * `.motion-deferred` in globals.css) and smooth scrolling start on the visitor's first
 * pointer, wheel, touch, key or scroll, so the first seconds after load are spent showing
 * the page, not animating it. The island demo is not deferred.
 */
export function useFirstInteractionRoot(): void {
  useEffect(() => {
    if (interacted) return
    EVENTS.forEach((e) => window.addEventListener(e, markInteracted, { passive: true }))
    return () => EVENTS.forEach((e) => window.removeEventListener(e, markInteracted))
  }, [])
}

/** True once the visitor has interacted with the page. */
export function useHasInteracted(): boolean {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => interacted,
    () => false,
  )
}
