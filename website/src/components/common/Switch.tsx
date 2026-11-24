import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

interface SwitchProps {
  label: ReactNode
  checked: boolean
  onChange: (v: boolean) => void
  dotClass?: string
  description?: ReactNode
  className?: string
}

/** Accessible switch row (role="switch"); the knob slides with transform. */
export function Switch({
  label,
  checked,
  onChange,
  dotClass,
  description,
  className,
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={cn(
        'flex w-full items-center justify-between gap-4 rounded-xl border border-hairline bg-surface-1 px-4 py-3 text-left text-sm transition-colors hover:border-hairline-strong',
        className,
      )}
    >
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="flex items-center gap-2.5">
          {dotClass && <span aria-hidden="true" className={cn('size-2 rounded-full', dotClass)} />}
          {label}
        </span>
        {description && <span className="text-xs text-fg-muted">{description}</span>}
      </span>
      <SwitchTrack checked={checked} />
    </button>
  )
}

export function SwitchTrack({ checked }: { checked: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'relative inline-flex h-6 w-10 shrink-0 items-center rounded-full transition-colors duration-200',
        checked ? 'bg-live-green' : 'bg-white/15',
      )}
    >
      <span
        className={cn(
          'absolute left-0.5 size-5 rounded-full bg-fg shadow transition-transform duration-300 ease-out-expo',
          checked ? 'translate-x-4' : 'translate-x-0',
        )}
      />
    </span>
  )
}
