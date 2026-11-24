import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { useEffect, useRef, useState } from 'react'
import { appCaptures, captureScreenWidth, type AppCapture } from '@/content/media'
import { usePageVisible } from '@/hooks/usePageVisible'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/cn'
import { asset } from '@/lib/format'
import { easeOutExpo } from '@/lib/motion'

interface DesktopPreviewProps {
  className?: string
  /** Seconds per capture. */
  interval?: number
  captures?: AppCapture[]
  /**
   * How far the view is zoomed into the top of the screen (1 = the whole 1920-px-wide
   * screen). Captures keep their true proportions relative to that screen.
   */
  zoom?: number
}

/**
 * The top of a neutral desktop with the real notch on it: captures of the app (from
 * `notchly --demo`) shown one after another. Stands in for the hero video until one is
 * recorded. Pauses offscreen and in background tabs; with reduced motion it shows the
 * first capture only.
 */
export function DesktopPreview({
  className,
  interval = 3.2,
  captures = appCaptures,
  zoom = 1.6,
}: DesktopPreviewProps) {
  const ref = useRef<HTMLDivElement>(null)
  const [index, setIndex] = useState(0)
  const [inView, setInView] = useState(false)
  const visible = usePageVisible()
  const reduced = useReducedMotion()

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const io = new IntersectionObserver(([e]) => setInView(Boolean(e?.isIntersecting)))
    io.observe(el)
    return () => io.disconnect()
  }, [])

  useEffect(() => {
    if (reduced || !inView || !visible || captures.length < 2) return
    const t = window.setTimeout(() => setIndex((i) => (i + 1) % captures.length), interval * 1000)
    return () => window.clearTimeout(t)
  }, [index, inView, visible, reduced, interval, captures.length])

  const shot = captures[index] ?? captures[0]
  if (!shot) return null

  return (
    <div
      ref={ref}
      className={cn('relative aspect-[12/5] w-full overflow-hidden bg-desktop', className)}
    >
      {/* The screen's top edge. */}
      <div className="absolute inset-x-0 top-0 h-px bg-white/15" />

      <AnimatePresence mode="popLayout" initial={false}>
        <m.picture
          key={shot.id}
          className="absolute top-0 left-1/2 block -translate-x-1/2"
          style={{ width: `${Math.min(96, (shot.width / captureScreenWidth) * 100 * zoom)}%` }}
          initial={{ opacity: 0, scale: 0.96, y: -6 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.98 }}
          transition={{ duration: 0.5, ease: easeOutExpo }}
        >
          <source type="image/webp" srcSet={asset(shot.webp)} />
          <img
            src={asset(shot.png)}
            alt={shot.alt}
            width={shot.width}
            height={shot.height}
            decoding="async"
            className="block h-auto w-full"
          />
        </m.picture>
      </AnimatePresence>

      <p className="absolute right-[2.5%] bottom-[5%] font-mono text-[10px] text-white/45 sm:text-[11px]">
        Real captures · notchly --demo
      </p>
    </div>
  )
}
