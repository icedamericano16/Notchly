import { TriangleAlert } from 'lucide-react'
import type { ComponentType } from 'react'
import { Link } from 'react-router'
import { notFoundIsland, type IslandActivityId } from '@/content/island'
import { CompactRow } from './parts'

/*
 * Views for the React island, which only renders site states the app never shows (the 404
 * page). Everything the site shows from the app is a real capture (content/media.ts).
 */

export type IslandSize = 'md' | 'sm'

interface Dims {
  w: number
  h: number
}

export interface ActivityView {
  /** Compact pill size per breakpoint. */
  size: Record<IslandSize, Dims>
  /** CSS colour of the soft glow under the island. */
  glow: string
  Compact: ComponentType
  Expanded: ComponentType
}

export const EXPANDED_WIDTH: Record<IslandSize, number> = { md: 400, sm: 320 }
export const COMPACT_HEIGHT: Record<IslandSize, number> = { md: 40, sm: 36 }

const dims = (md: number, sm: number): Record<IslandSize, Dims> => ({
  md: { w: md, h: COMPACT_HEIGHT.md },
  sm: { w: sm, h: COMPACT_HEIGHT.sm },
})

function NotFoundCompact() {
  return (
    <CompactRow
      left={
        <>
          <TriangleAlert className="size-4 text-live-orange" aria-hidden="true" />
          <span className="truncate text-[13px]">{notFoundIsland.title}</span>
        </>
      }
      right={<span className="font-mono text-[13px] text-live-orange">{notFoundIsland.code}</span>}
    />
  )
}

function NotFoundExpanded() {
  return (
    <div className="flex items-center justify-between gap-4 p-5 pr-12">
      <div>
        <p className="font-mono text-[12px] text-live-orange">Error {notFoundIsland.code}</p>
        <p className="text-[17px] font-medium">{notFoundIsland.title}</p>
        <p className="text-[13px] text-fg-muted">This page doesn't exist.</p>
      </div>
      <Link
        to="/"
        className="rounded-full bg-fg px-4 py-2 text-[13px] font-medium text-canvas transition-colors hover:bg-fg/90"
      >
        Go home
      </Link>
    </div>
  )
}

function IdleCompact() {
  return null
}

function IdleExpanded() {
  return null
}

export const activities: Record<IslandActivityId, ActivityView> = {
  idle: { size: dims(124, 112), glow: 'transparent', Compact: IdleCompact, Expanded: IdleExpanded },
  notFound: {
    size: dims(260, 232),
    glow: 'var(--color-live-orange)',
    Compact: NotFoundCompact,
    Expanded: NotFoundExpanded,
  },
}
