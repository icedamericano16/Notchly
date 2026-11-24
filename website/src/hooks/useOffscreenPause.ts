import { useEffect, useRef } from 'react'

/**
 * Pauses every CSS animation inside the element while it's offscreen, by toggling
 * `data-offscreen` (see globals.css). Decorative loops then cost nothing below the fold.
 */
export function useOffscreenPause<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const io = new IntersectionObserver(
      ([entry]) => {
        el.dataset.offscreen = entry && entry.isIntersecting ? 'false' : 'true'
      },
      { rootMargin: '100px' },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [])
  return ref
}
