import * as m from 'motion/react-m'
import type { ReactNode } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { reducedRevealVariants, revealVariants, staggerChildren } from '@/lib/motion'

const viewport = { once: true, margin: '0px 0px -12% 0px' } as const

interface RevealProps {
  children: ReactNode
  className?: string
  /** Seconds. */
  delay?: number
}

/** Fade + 12px rise + slight blur when scrolled into view; plain fade with reduced motion. */
export function Reveal({ children, className, delay = 0 }: RevealProps) {
  const reduced = useReducedMotion()
  return (
    <m.div
      className={className}
      variants={reduced ? reducedRevealVariants : revealVariants}
      initial="hidden"
      whileInView="visible"
      viewport={viewport}
      transition={{ delay }}
    >
      {children}
    </m.div>
  )
}

interface RevealGroupProps {
  children: ReactNode
  className?: string
  stagger?: number
  delay?: number
  as?: 'div' | 'ul' | 'ol'
}

/** Staggers its <RevealItem> children in as the group enters the viewport. */
export function RevealGroup({
  children,
  className,
  stagger = 0.08,
  delay = 0,
  as = 'div',
}: RevealGroupProps) {
  const Comp = as === 'ul' ? m.ul : as === 'ol' ? m.ol : m.div
  return (
    <Comp
      className={className}
      variants={staggerChildren(stagger, delay)}
      initial="hidden"
      whileInView="visible"
      viewport={viewport}
    >
      {children}
    </Comp>
  )
}

export function RevealItem({
  children,
  className,
  as = 'div',
}: {
  children: ReactNode
  className?: string
  as?: 'div' | 'li'
}) {
  const reduced = useReducedMotion()
  const Comp = as === 'li' ? m.li : m.div
  return (
    <Comp className={className} variants={reduced ? reducedRevealVariants : revealVariants}>
      {children}
    </Comp>
  )
}
