import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

interface PageHeaderProps {
  eyebrow?: string
  title: string
  description?: ReactNode
  children?: ReactNode
  className?: string
}

/** Top of an inner page: CSS entrance (paints immediately), big tight title. */
export function PageHeader({ eyebrow, title, description, children, className }: PageHeaderProps) {
  return (
    <header className={cn('relative isolate overflow-hidden pt-36 pb-12 sm:pt-44', className)}>
      <div
        aria-hidden="true"
        className="absolute top-0 left-1/2 -z-10 h-72 w-[48rem] max-w-[95vw] -translate-x-1/2 rounded-full bg-[radial-gradient(closest-side,rgb(91_108_255/0.22),transparent)]"
      />
      <div className="container-page">
        {eyebrow && (
          <p className="animate-fade-in font-mono text-xs tracking-[0.18em] text-fg-muted uppercase">
            {eyebrow}
          </p>
        )}
        <h1
          className="mt-4 max-w-3xl animate-rise-in text-5xl text-display sm:text-7xl"
          style={{ animationDelay: '0.05s' }}
        >
          {title}
        </h1>
        {description && (
          <p
            className="mt-6 max-w-2xl animate-rise-in text-lg leading-relaxed text-pretty text-fg-muted"
            style={{ animationDelay: '0.15s' }}
          >
            {description}
          </p>
        )}
        {children && (
          <div className="mt-8 animate-rise-in" style={{ animationDelay: '0.25s' }}>
            {children}
          </div>
        )}
      </div>
    </header>
  )
}
