/*
 * CountUp — after React Bits' CountUp (reactbits.dev, MIT). Counts from `from` to `to` once
 * it scrolls into view. Writes textContent directly (no re-render per frame); with reduced
 * motion it shows the final value at once.
 */
import { useEffect, useRef } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'

interface CountUpProps {
  to: number
  from?: number
  /** Seconds. */
  duration?: number
  decimals?: number
  separator?: string
  className?: string
}

function format(n: number, decimals: number, separator: string): string {
  const fixed = n.toFixed(decimals)
  const [int = '0', frac] = fixed.split('.')
  const grouped = separator ? int.replace(/\B(?=(\d{3})+(?!\d))/g, separator) : int
  return frac ? `${grouped}.${frac}` : grouped
}

export function CountUp({
  to,
  from = 0,
  duration = 1.6,
  decimals = 0,
  separator = ',',
  className,
}: CountUpProps) {
  const ref = useRef<HTMLSpanElement>(null)
  const reduced = useReducedMotion()

  useEffect(() => {
    const el = ref.current
    if (!el) return
    if (reduced) {
      el.textContent = format(to, decimals, separator)
      return
    }
    let raf = 0
    const run = () => {
      const start = performance.now()
      const tick = (now: number) => {
        const t = Math.min(1, (now - start) / (duration * 1000))
        const eased = 1 - Math.pow(1 - t, 4)
        el.textContent = format(from + (to - from) * eased, decimals, separator)
        if (t < 1) raf = requestAnimationFrame(tick)
      }
      raf = requestAnimationFrame(tick)
    }
    const io = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          io.disconnect()
          run()
        }
      },
      { threshold: 0.6 },
    )
    io.observe(el)
    return () => {
      io.disconnect()
      cancelAnimationFrame(raf)
    }
  }, [to, from, duration, decimals, separator, reduced])

  return (
    <span ref={ref} className={className}>
      {format(reduced ? to : from, decimals, separator)}
    </span>
  )
}
