import { Pause, Play } from 'lucide-react'
import { useInView } from 'motion/react'
import { useRef, useState } from 'react'
import { heroSequence, islandMeta, type IslandActivityId } from '@/content/island'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import { usePageVisible } from '@/hooks/usePageVisible'
import { cn } from '@/lib/cn'
import { DynamicIsland } from './DynamicIsland'
import { useIslandCycle } from './useIslandCycle'

interface IslandDemoProps {
  sequence?: readonly IslandActivityId[]
  intervalMs?: number
  showControls?: boolean
  className?: string
}

/**
 * The hero centrepiece: the island cycling through live activities. It pauses while
 * expanded, offscreen, in a background tab, or when the visitor presses pause (WCAG 2.2.2).
 * The island floats over what's below, like the real notch.
 */
export function IslandDemo({
  sequence = heroSequence,
  intervalMs = 3800,
  showControls = true,
  className,
}: IslandDemoProps) {
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref)
  const visible = usePageVisible()
  const small = useMediaQuery('(max-width: 639px)')
  const [expanded, setExpanded] = useState(false)
  const [userPaused, setUserPaused] = useState(false)
  const paused = userPaused || expanded || !inView || !visible
  const { activity, index, select } = useIslandCycle(sequence, { intervalMs, paused })
  const size = small ? 'sm' : 'md'

  return (
    // relative + z-30 on the root: an entrance animation on it would otherwise trap the
    // island's z-index inside its own stacking context, under the headline.
    <div ref={ref} className={cn('relative z-30 flex flex-col items-center gap-5', className)}>
      {/* Reserves the compact height; the island grows over the content below. */}
      {/* Height from CSS (36px / 40px = COMPACT_HEIGHT), so prerendered HTML and the
          hydrated page agree before the media query is known: no layout shift. */}
      <div className="relative z-30 h-9 w-full sm:h-10">
        <div className="absolute top-0 left-1/2 -translate-x-1/2">
          <DynamicIsland
            activity={activity}
            expanded={expanded}
            onExpandedChange={setExpanded}
            size={size}
          />
        </div>
      </div>

      {showControls && (
        <div
          role="group"
          aria-label="Island demo controls"
          className="flex items-center gap-3 rounded-full border border-hairline bg-surface-1 py-1.5 pr-1.5 pl-3"
        >
          <div className="flex items-center gap-1.5">
            {sequence.map((id, i) => (
              <button
                key={id}
                type="button"
                aria-label={`Show ${islandMeta[id].label}`}
                aria-pressed={i === index}
                title={islandMeta[id].label}
                onClick={() => select(i)}
                className="group flex h-5 items-center px-0.5"
              >
                <span className="relative block h-1 w-5 overflow-hidden rounded-full bg-white/15 transition-colors group-hover:bg-white/25">
                  {i === index && (
                    <span
                      key={`${index}-${paused}`}
                      className="absolute inset-0 origin-left rounded-full bg-island-accent"
                      style={{
                        animation: paused
                          ? undefined
                          : `dot-progress ${intervalMs}ms linear forwards`,
                        transform: paused ? 'scaleX(1)' : undefined,
                      }}
                    />
                  )}
                </span>
              </button>
            ))}
          </div>
          <span className="text-[12px] text-fg-muted tabular-nums" aria-live="off">
            {islandMeta[activity].label}
          </span>
          <button
            type="button"
            onClick={() => setUserPaused((p) => !p)}
            aria-label={userPaused ? 'Play the island demo' : 'Pause the island demo'}
            className="flex size-6 items-center justify-center rounded-full bg-white/[0.08] text-fg-muted transition-colors hover:bg-white/15 hover:text-fg"
          >
            {userPaused ? (
              <Play className="size-3 fill-current" aria-hidden="true" />
            ) : (
              <Pause className="size-3 fill-current" aria-hidden="true" />
            )}
          </button>
        </div>
      )}
    </div>
  )
}
