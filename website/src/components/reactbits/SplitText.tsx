/*
 * SplitText — after React Bits' SplitText (reactbits.dev, MIT), rewritten without GSAP:
 * each word rises in with a CSS animation, so the hero headline (the LCP element) paints
 * on the first frame without waiting for any JavaScript animation library.
 */
import type { CSSProperties, ElementType } from 'react'
import { cn } from '@/lib/cn'

interface SplitTextProps {
  text: string
  /** A trailing part of `text` drawn with the accent gradient. */
  accent?: string
  as?: ElementType
  id?: string
  className?: string
  /** Seconds before the first word. */
  delay?: number
  /** Seconds between words. */
  stagger?: number
}

export function SplitText({
  text,
  accent,
  as: Tag = 'p',
  id,
  className,
  delay = 0,
  stagger = 0.06,
}: SplitTextProps) {
  const words = text.split(' ')
  const accentCount = accent && text.endsWith(accent) ? accent.split(' ').length : 0
  const firstAccent = words.length - accentCount

  return (
    <Tag id={id} className={className}>
      {words.map((word, i) => {
        const isAccent = accentCount > 0 && i >= firstAccent
        const style: CSSProperties = { animationDelay: `${delay + i * stagger}s` }
        if (isAccent && accentCount > 1) {
          // One continuous gradient across the accent words.
          style.backgroundSize = `${accentCount * 100}% 100%`
          style.backgroundPosition = `${((i - firstAccent) / (accentCount - 1)) * 100}% 0`
        }
        return (
          <span key={`${word}-${i}`}>
            <span
              className={cn(
                'inline-block animate-rise-in pb-[0.08em]',
                isAccent && 'text-brand-gradient',
              )}
              style={style}
            >
              {word}
            </span>
            {i < words.length - 1 && ' '}
          </span>
        )
      })}
    </Tag>
  )
}
