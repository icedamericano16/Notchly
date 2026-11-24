import type { CSSProperties } from 'react'
import { cn } from '@/lib/cn'

interface BorderBeamProps {
  /** Seconds per lap. */
  duration?: number
  className?: string
}

/**
 * After Magic UI's Border Beam: a light travels around the parent's border. The border is a
 * static mask; inside it a conic gradient rotates (transform only). The parent needs
 * `relative` and a border radius.
 */
export function BorderBeam({ duration = 8, className }: BorderBeamProps) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'motion-deferred pointer-events-none absolute inset-0 overflow-hidden rounded-[inherit] p-px [-webkit-mask-composite:xor] [mask:linear-gradient(#000_0_0)_content-box_exclude,linear-gradient(#000_0_0)]',
        className,
      )}
    >
      <span
        className="absolute top-1/2 left-1/2 aspect-square w-[160%] -translate-x-1/2 -translate-y-1/2"
        style={{ '--beam-duration': `${duration}s` } as CSSProperties}
      >
        <span className="block size-full animate-beam bg-[conic-gradient(from_0deg,transparent_0deg,var(--color-brand-blue)_40deg,var(--color-brand-violet)_70deg,transparent_100deg)]" />
      </span>
    </span>
  )
}
