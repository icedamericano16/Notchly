import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

/** Five bars bouncing with transform only. Colours come from the island accent. */
export function Waveform({
  playing = true,
  className,
  bars = 5,
}: {
  playing?: boolean
  className?: string
  bars?: number
}) {
  return (
    <span aria-hidden="true" className={cn('flex h-4 items-center gap-[3px]', className)}>
      {Array.from({ length: bars }, (_, i) => (
        <span
          key={i}
          className={cn(
            'h-full w-[3px] origin-center rounded-full bg-island-accent',
            playing ? 'animate-eq' : 'scale-y-[0.3]',
          )}
          style={{
            animationDelay: `${(i * 0.17) % 0.6}s`,
            animationDuration: `${0.9 + (i % 3) * 0.2}s`,
          }}
        />
      ))}
    </span>
  )
}

/** Rounded "album art": the accent gradient with a soft highlight. */
export function AlbumArt({ className, children }: { className?: string; children?: ReactNode }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'relative flex shrink-0 items-center justify-center overflow-hidden rounded-md bg-island-accent',
        className,
      )}
    >
      <span className="absolute inset-0 bg-gradient-to-br from-white/30 to-transparent" />
      {children}
    </span>
  )
}

/** Compact layout: content hugs the two ends, like a real notch around a camera. */
export function CompactRow({ left, right }: { left: ReactNode; right: ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-between gap-3 pr-3 pl-2">
      <div className="flex min-w-0 items-center gap-2">{left}</div>
      <div className="flex shrink-0 items-center gap-2">{right}</div>
    </div>
  )
}

/** A horizontal progress bar that fills with scaleX (no layout animation). */
export function ProgressBar({
  value,
  className,
  accent = true,
}: {
  value: number
  className?: string
  accent?: boolean
}) {
  return (
    <span
      aria-hidden="true"
      className={cn('block h-1 w-full overflow-hidden rounded-full bg-white/10', className)}
    >
      <span
        className={cn(
          'block h-full w-full origin-left rounded-full transition-transform duration-1000 ease-linear',
          accent ? 'bg-island-accent' : 'bg-fg',
        )}
        style={{ transform: `scaleX(${Math.min(1, Math.max(0, value))})` }}
      />
    </span>
  )
}

interface RoundButtonProps {
  label: string
  onClick?: () => void
  children: ReactNode
  tone?: 'neutral' | 'green' | 'red' | 'orange' | 'accent'
  size?: 'sm' | 'md' | 'lg'
  pressed?: boolean
}

const tones: Record<NonNullable<RoundButtonProps['tone']>, string> = {
  neutral: 'bg-white/10 text-fg hover:bg-white/15',
  green: 'bg-live-green text-canvas hover:brightness-110',
  red: 'bg-live-red text-fg hover:brightness-110',
  orange: 'bg-live-orange/20 text-live-orange hover:bg-live-orange/30',
  accent: 'bg-fg text-canvas hover:bg-fg/90',
}

const sizes = {
  sm: 'size-7 [&_svg]:size-3.5',
  md: 'size-9 [&_svg]:size-4',
  lg: 'size-11 [&_svg]:size-5',
}

/** Circular icon button used inside the expanded island. */
export function RoundButton({
  label,
  onClick,
  children,
  tone = 'neutral',
  size = 'md',
  pressed,
}: RoundButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      onClick={onClick}
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-full transition-[background-color,filter,transform] duration-200 active:scale-95',
        tones[tone],
        sizes[size],
      )}
    >
      {children}
    </button>
  )
}
