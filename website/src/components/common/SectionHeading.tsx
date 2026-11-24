import type { ReactNode } from 'react'
import { BlurText } from '@/components/reactbits/BlurText'
import { cn } from '@/lib/cn'
import { Reveal } from './Reveal'

interface SectionHeadingProps {
  eyebrow?: string
  title: string
  description?: ReactNode
  align?: 'center' | 'start'
  id?: string
  className?: string
}

/** Eyebrow + big tight title (words blur in) + muted description. */
export function SectionHeading({
  eyebrow,
  title,
  description,
  align = 'center',
  id,
  className,
}: SectionHeadingProps) {
  const center = align === 'center'
  return (
    <div className={cn('flex flex-col gap-4', center && 'items-center text-center', className)}>
      {eyebrow && (
        <Reveal>
          <p className="font-mono text-xs tracking-[0.18em] text-fg-muted uppercase">{eyebrow}</p>
        </Reveal>
      )}
      <h2 id={id} className="max-w-3xl text-4xl text-display sm:text-5xl lg:text-6xl">
        <BlurText as="span" text={title} />
      </h2>
      {description && (
        <Reveal delay={0.15}>
          <p
            className={cn(
              'max-w-2xl text-lg leading-relaxed text-pretty text-fg-muted',
              center && 'mx-auto',
            )}
          >
            {description}
          </p>
        </Reveal>
      )}
    </div>
  )
}
