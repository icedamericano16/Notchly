import { useId } from 'react'
import { cn } from '@/lib/cn'

interface LogoMarkProps {
  className?: string
}

/** The app icon: a true-black pill with the blue → violet light bar. */
export function LogoMark({ className }: LogoMarkProps) {
  const gradientId = useId()
  return (
    <svg
      viewBox="0 0 40 26"
      className={cn('h-6 w-auto', className)}
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" x2="1" y1="0" y2="0">
          <stop offset="0" style={{ stopColor: 'var(--color-brand-blue)' }} />
          <stop offset="1" style={{ stopColor: 'var(--color-brand-violet)' }} />
        </linearGradient>
      </defs>
      <rect
        x="0.5"
        y="0.5"
        width="39"
        height="25"
        rx="12.5"
        style={{ fill: 'var(--color-island)', stroke: 'var(--color-hairline-strong)' }}
      />
      <rect x="10" y="15.5" width="20" height="3" rx="1.5" fill={`url(#${gradientId})`} />
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
