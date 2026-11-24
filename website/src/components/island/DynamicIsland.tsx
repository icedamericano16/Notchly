import { X } from 'lucide-react'
import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import {
  useEffect,
  useEffectEvent,
  useRef,
  type CSSProperties,
  type FocusEvent,
  type KeyboardEvent,
  type PointerEvent,
} from 'react'
import { islandMeta, type IslandActivityId } from '@/content/island'
import { cn } from '@/lib/cn'
import { islandSpring } from '@/lib/motion'
import { activities, EXPANDED_WIDTH, type IslandSize } from './activities'

export interface DynamicIslandProps {
  activity: IslandActivityId
  expanded?: boolean
  onExpandedChange?: (expanded: boolean) => void
  /** Hover, focus, tap and keyboard open it. Off for illustrations. */
  interactive?: boolean
  size?: IslandSize
  className?: string
  /** Theme preview: override the accent gradient stops. */
  accent?: readonly [string, string, string]
}

const EXPANDED_RADIUS = 30

/**
 * A React island for site-only states (the 404 page): a true-black pill that morphs with
 * one spring (Motion `layout`, so only transforms animate) and opens on hover, focus or
 * tap. It never shows app data: app states on the site are real captures.
 */
export function DynamicIsland({
  activity,
  expanded = false,
  onExpandedChange,
  interactive = true,
  size = 'md',
  className,
  accent,
}: DynamicIslandProps) {
  const rootRef = useRef<HTMLDivElement>(null)
  const toggleRef = useRef<HTMLButtonElement>(null)
  const leaveTimer = useRef<number | undefined>(undefined)

  const view = activities[activity]
  const meta = islandMeta[activity]
  const compact = view.size[size]
  const width = expanded ? EXPANDED_WIDTH[size] : compact.w
  const height = expanded ? undefined : compact.h
  const radius = expanded ? EXPANDED_RADIUS : compact.h / 2

  const setExpanded = (next: boolean) => {
    if (next !== expanded) onExpandedChange?.(next)
  }
  const closeFromOutside = useEffectEvent(() => onExpandedChange?.(false))

  // A tap or click anywhere else closes it (touch has no "pointer leave").
  useEffect(() => {
    if (!expanded || !interactive) return
    const onDown = (e: globalThis.PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) closeFromOutside()
    }
    document.addEventListener('pointerdown', onDown)
    return () => document.removeEventListener('pointerdown', onDown)
  }, [expanded, interactive])

  useEffect(() => () => window.clearTimeout(leaveTimer.current), [])

  const handlers = interactive
    ? {
        onPointerEnter: (e: PointerEvent) => {
          if (e.pointerType !== 'mouse') return
          window.clearTimeout(leaveTimer.current)
          setExpanded(true)
        },
        onPointerLeave: (e: PointerEvent) => {
          if (e.pointerType !== 'mouse') return
          leaveTimer.current = window.setTimeout(() => setExpanded(false), 160)
        },
        onKeyDown: (e: KeyboardEvent) => {
          if (e.key === 'Escape' && expanded) {
            e.stopPropagation()
            setExpanded(false)
            toggleRef.current?.focus()
          }
        },
        onBlur: (e: FocusEvent) => {
          if (!rootRef.current?.contains(e.relatedTarget as Node | null)) setExpanded(false)
        },
      }
    : {}

  const accentStyle = accent
    ? ({
        '--island-a': accent[0],
        '--island-b': accent[1],
        '--island-c': accent[2],
      } as CSSProperties)
    : undefined

  return (
    <div
      ref={rootRef}
      className={cn('relative text-left', className)}
      style={accentStyle}
      {...handlers}
    >
      {/* Soft glow in the activity's colour. Opacity only. */}
      <m.div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-6 top-1 -z-10 h-10 rounded-full blur-2xl"
        style={{ background: view.glow }}
        initial={false}
        animate={{ opacity: activity === 'idle' ? 0 : expanded ? 0.35 : 0.5 }}
        transition={{ duration: 0.5 }}
      />

      <m.div
        layout
        transition={islandSpring}
        role="group"
        aria-roledescription="Dynamic Island"
        aria-label={`Notchly island. ${meta.description}`}
        data-activity={activity}
        data-expanded={expanded}
        className="relative overflow-hidden bg-island text-fg shadow-island ring-1 ring-white/[0.07]"
        style={{ width, height, borderRadius: radius }}
      >
        <AnimatePresence mode="popLayout" initial={false}>
          <m.div
            key={`${activity}:${expanded ? 'open' : 'compact'}`}
            layout
            className={expanded ? 'w-full' : 'h-full w-full'}
            initial={{ opacity: 0, scale: 0.92, filter: 'blur(6px)' }}
            animate={{
              opacity: 1,
              scale: 1,
              filter: 'blur(0px)',
              transition: { ...islandSpring, opacity: { duration: 0.25, delay: 0.08 } },
            }}
            exit={{ opacity: 0, scale: 0.96, filter: 'blur(4px)', transition: { duration: 0.14 } }}
          >
            {expanded ? (
              <>
                <view.Expanded />
              </>
            ) : (
              <view.Compact />
            )}
          </m.div>
        </AnimatePresence>

        {interactive && (
          <button
            ref={toggleRef}
            type="button"
            aria-expanded={expanded}
            aria-label={expanded ? 'Collapse the island' : `${meta.label}. Open the control center`}
            onClick={() => setExpanded(!expanded)}
            className={cn(
              'z-10 rounded-[inherit] transition-colors duration-200',
              expanded
                ? 'absolute top-3 right-3 flex size-7 items-center justify-center rounded-full bg-white/10 text-fg-muted hover:bg-white/15 hover:text-fg'
                : 'absolute inset-0 cursor-pointer',
            )}
          >
            {expanded && <X className="size-3.5" aria-hidden="true" />}
          </button>
        )}
      </m.div>
    </div>
  )
}
