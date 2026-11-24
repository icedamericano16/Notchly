import { useId } from 'react'
import { cn } from '@/lib/cn'

/* The Notchly mark (assets/brand/notchly-logo-animated.svg): a violet squircle with a
   true-black notch whose two eyes blink every few seconds. The blink is CSS (`.logo-eye`
   in globals.css) and stops with reduced motion. Brand colours live in @theme. */
const SQUIRCLE =
  'M308.0192,0 H715.9808 C999.936,0 1024,24.064 1024,308.0192 V715.9808 C1024,999.936 999.936,1024 715.9808,1024 H308.0192 C24.064,1024 0,999.936 0,715.9808 V308.0192 C0,24.064 24.064,0 308.0192,0 Z'

interface LogoMarkProps {
  className?: string
  /** Blink the eyes (on by default). */
  animated?: boolean
}

export function LogoMark({ className, animated = true }: LogoMarkProps) {
  const gradientId = useId()
  return (
    <svg
      viewBox="0 0 1024 1024"
      className={cn('size-7 shrink-0', className)}
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" style={{ stopColor: 'var(--color-logo-from)' }} />
          <stop offset="1" style={{ stopColor: 'var(--color-logo-to)' }} />
        </linearGradient>
      </defs>
      <path d={SQUIRCLE} fill={`url(#${gradientId})`} />
      <rect
        x="123"
        y="302.82"
        width="778"
        height="275"
        rx="137.5"
        style={{ fill: 'var(--color-logo-notch)' }}
      />
      {[364, 576].map((x) => (
        <rect
          key={x}
          className={animated ? 'logo-eye' : undefined}
          x={x}
          y="383.32"
          width="84"
          height="114"
          rx="42"
          style={{ fill: 'var(--color-logo-eye)' }}
        />
      ))}
    </svg>
  )
}

interface LogoProps {
  className?: string
}

/** Mark + wordmark. */
export function Logo({ className }: LogoProps) {
  return (
    <span className={cn('inline-flex items-center gap-2.5', className)}>
      <LogoMark />
      <span className="text-[15px] font-semibold tracking-tightish">Notchly</span>
    </span>
  )
}
