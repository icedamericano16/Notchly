import type { CSSProperties, ReactNode } from 'react'
import { useOffscreenPause } from '@/hooks/useOffscreenPause'
import { cn } from '@/lib/cn'

interface MarqueeProps {
  children: ReactNode
  /** Seconds per loop. */
  duration?: number
  className?: string
  /** Accessible name; the duplicate copy is hidden from assistive tech. */
  label?: string
}

/**
 * After Magic UI's Marquee: two copies of the row slide left forever (translateX only),
 * pause on hover and offscreen, and fade out at the edges. With reduced motion the row
 * stays still and wraps.
 */
export function Marquee({ children, duration = 40, className, label }: MarqueeProps) {
  const ref = useOffscreenPause<HTMLDivElement>()
  return (
    <div
      ref={ref}
      role={label ? 'region' : undefined}
      aria-label={label}
      className={cn(
        'motion-deferred group relative flex overflow-hidden [mask-image:linear-gradient(90deg,transparent,#000_12%,#000_88%,transparent)] motion-reduce:[mask-image:none]',
        className,
      )}
      style={{ '--marquee-duration': `${duration}s` } as CSSProperties}
    >
      <div className="flex w-max shrink-0 animate-marquee group-hover:[animation-play-state:paused] motion-reduce:w-full motion-reduce:animate-none motion-reduce:flex-wrap motion-reduce:justify-center">
        <div className="flex shrink-0 items-center motion-reduce:flex-wrap motion-reduce:justify-center">
          {children}
        </div>
        <div aria-hidden="true" className="flex shrink-0 items-center motion-reduce:hidden">
          {children}
        </div>
      </div>
    </div>
  )
}
