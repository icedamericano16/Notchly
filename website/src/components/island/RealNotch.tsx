import { Pause, Play } from 'lucide-react'
import { AnimatePresence, useInView } from 'motion/react'
import * as m from 'motion/react-m'
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react'
import { CaptureImage } from '@/components/common/CaptureImage'
import { captures, heroNotchCaptures, type AppCapture } from '@/content/media'
import { usePageVisible } from '@/hooks/usePageVisible'
import { cn } from '@/lib/cn'
import { islandSpring } from '@/lib/motion'

/** Short names for the controls, by capture id. */
const labels: Record<string, string> = {
  'pill-music': 'Music',
  'pill-timer': 'Timer',
  'popup-download': 'Download',
  'popup-capslock': 'Caps Lock',
  'popup-brightness': 'Brightness',
  'popup-charging': 'Charging',
  'pill-idle': 'At rest',
}

interface RealNotchProps {
  sequence?: AppCapture[]
  /** Seconds per state. */
  interval?: number
  className?: string
}

/**
 * The hero's notch: real captures of Notchly's collapsed states and pop-ups, at the size
 * they have on screen, one after another. Hover, focus or tap opens it into the real
 * Control Center. Pauses while open, offscreen, in background tabs, or with the pause
 * button (WCAG 2.2.2).
 */
export function RealNotch({
  sequence = heroNotchCaptures,
  interval = 3,
  className,
}: RealNotchProps) {
  const ref = useRef<HTMLDivElement>(null)
  const leaveTimer = useRef<number | undefined>(undefined)
  const inView = useInView(ref)
  const visible = usePageVisible()
  const [index, setIndex] = useState(0)
  const [expanded, setExpanded] = useState(false)
  const [userPaused, setUserPaused] = useState(false)
  const paused = userPaused || expanded || !inView || !visible

  useEffect(() => {
    if (paused || sequence.length < 2) return
    const t = window.setTimeout(() => setIndex((i) => (i + 1) % sequence.length), interval * 1000)
    return () => window.clearTimeout(t)
  }, [index, paused, interval, sequence.length])

  // A tap anywhere else closes it (touch has no "pointer leave").
  useEffect(() => {
    if (!expanded) return
    const onDown = (e: globalThis.PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setExpanded(false)
    }
    document.addEventListener('pointerdown', onDown)
    return () => document.removeEventListener('pointerdown', onDown)
  }, [expanded])
  useEffect(() => () => window.clearTimeout(leaveTimer.current), [])

  const current = sequence[index % sequence.length] ?? sequence[0]
  if (!current) return null
  const shown = expanded ? captures.controlCenter : current

  const enter = (e: PointerEvent) => {
    if (e.pointerType !== 'mouse') return
    window.clearTimeout(leaveTimer.current)
    setExpanded(true)
  }
  const leave = (e: PointerEvent) => {
    if (e.pointerType !== 'mouse') return
    leaveTimer.current = window.setTimeout(() => setExpanded(false), 160)
  }
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && expanded) setExpanded(false)
  }

  return (
    <div ref={ref} className={cn('relative z-30 flex flex-col items-center gap-4', className)}>
      {/* Reserves room for the tallest pop-up; the open notch grows over what's below. */}
      <div className="relative h-[5.5rem] w-full">
        <div
          className="absolute top-0 left-1/2 w-max max-w-[calc(100vw-2rem)] -translate-x-1/2 text-left"
          onPointerEnter={enter}
          onPointerLeave={leave}
          onKeyDown={onKey}
        >
          <button
            type="button"
            aria-expanded={expanded}
            aria-label={
              expanded
                ? 'Close the Control Center'
                : `${labels[current.id] ?? 'Notchly'}: ${current.alt} Open the Control Center`
            }
            onClick={() => setExpanded((v) => !v)}
            className="block rounded-b-[1.4rem] focus-visible:outline-offset-4"
          >
            <AnimatePresence mode="popLayout" initial={false}>
              <m.div
                key={shown.id}
                className="origin-top"
                initial={{ opacity: 0, scale: 0.9, y: -4 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.96, transition: { duration: 0.15 } }}
                transition={islandSpring}
              >
                <CaptureImage
                  capture={shown}
                  priority
                  className="drop-shadow-[0_18px_40px_rgb(0_0_0/0.7)]"
                />
              </m.div>
            </AnimatePresence>
          </button>
        </div>
      </div>

      <div
        role="group"
        aria-label="Notch demo controls"
        className="flex items-center gap-3 rounded-full border border-hairline bg-surface-1 py-1.5 pr-1.5 pl-3"
      >
        <div className="flex items-center gap-1.5">
          {sequence.map((c, i) => (
            <button
              key={c.id}
              type="button"
              aria-label={`Show ${labels[c.id] ?? c.id}`}
              aria-pressed={i === index}
              title={labels[c.id]}
              onClick={() => setIndex(i)}
              className="group flex h-5 items-center px-0.5"
            >
              <span className="relative block h-1 w-4 overflow-hidden rounded-full bg-white/15 transition-colors group-hover:bg-white/25">
                {i === index && (
                  <span
                    key={`${index}-${paused}`}
                    className="absolute inset-0 origin-left rounded-full bg-fg"
                    style={{
                      animation: paused ? undefined : `dot-progress ${interval}s linear forwards`,
                      transform: paused ? 'scaleX(1)' : undefined,
                    }}
                  />
                )}
              </span>
            </button>
          ))}
        </div>
        <span className="min-w-[4.5rem] text-[12px] text-fg-muted">
          {expanded ? 'Control Center' : (labels[current.id] ?? '')}
        </span>
        <button
          type="button"
          onClick={() => setUserPaused((p) => !p)}
          aria-label={userPaused ? 'Play the notch demo' : 'Pause the notch demo'}
          className="flex size-6 items-center justify-center rounded-full bg-white/[0.08] text-fg-muted transition-colors hover:bg-white/15 hover:text-fg"
        >
          {userPaused ? (
            <Play className="size-3 fill-current" aria-hidden="true" />
          ) : (
            <Pause className="size-3 fill-current" aria-hidden="true" />
          )}
        </button>
      </div>
    </div>
  )
}
