/*
 * Magnet — after React Bits' Magnet (reactbits.dev, MIT): the child leans toward the
 * pointer while it's nearby. Transform only; off for touch and reduced motion.
 */
import { useRef, type PointerEvent, type ReactNode } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/cn'

interface MagnetProps {
  children: ReactNode
  /** Extra hit area around the child, px. */
  padding?: number
  /** Higher = weaker pull. */
  strength?: number
  className?: string
}

export function Magnet({ children, padding = 60, strength = 4, className }: MagnetProps) {
  const inner = useRef<HTMLDivElement>(null)
  const reduced = useReducedMotion()

  const move = (e: PointerEvent<HTMLDivElement>) => {
    if (reduced || e.pointerType !== 'mouse' || !inner.current) return
    const r = e.currentTarget.getBoundingClientRect()
    const x = (e.clientX - (r.left + r.width / 2)) / strength
    const y = (e.clientY - (r.top + r.height / 2)) / strength
    inner.current.style.transition = 'transform 0.25s var(--ease-out-quint)'
    inner.current.style.transform = `translate3d(${x}px, ${y}px, 0)`
  }
  const leave = () => {
    if (!inner.current) return
    inner.current.style.transition = 'transform 0.6s var(--ease-out-expo)'
    inner.current.style.transform = 'translate3d(0, 0, 0)'
  }

  return (
    <div
      className={cn('inline-block', className)}
      style={{ padding, margin: -padding }}
      onPointerMove={move}
      onPointerLeave={leave}
    >
      <div ref={inner} className="will-change-transform">
        {children}
      </div>
    </div>
  )
}
