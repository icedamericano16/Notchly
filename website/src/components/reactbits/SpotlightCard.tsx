/*
 * SpotlightCard — after React Bits' SpotlightCard (reactbits.dev, MIT). The light is a
 * pre-rendered radial gradient moved with `transform` (no per-frame repaint), listening
 * only while the pointer is over the card, and it fades with opacity.
 */
import { useRef, type ComponentProps, type CSSProperties, type PointerEvent } from 'react'
import { cn } from '@/lib/cn'

interface SpotlightCardProps extends ComponentProps<'div'> {
  /** Any CSS colour; defaults to a soft white. */
  spotlightColor?: string
  /** Diameter of the light, px. */
  size?: number
}

export function SpotlightCard({
  spotlightColor = 'rgb(255 255 255 / 0.10)',
  size = 420,
  className,
  children,
  style,
  onPointerMove,
  ...rest
}: SpotlightCardProps) {
  const lightRef = useRef<HTMLSpanElement>(null)
  const frame = useRef(0)

  const handleMove = (e: PointerEvent<HTMLDivElement>) => {
    onPointerMove?.(e)
    if (e.pointerType !== 'mouse') return
    const rect = e.currentTarget.getBoundingClientRect()
    const x = e.clientX - rect.left - size / 2
    const y = e.clientY - rect.top - size / 2
    cancelAnimationFrame(frame.current)
    frame.current = requestAnimationFrame(() => {
      if (lightRef.current) lightRef.current.style.transform = `translate3d(${x}px, ${y}px, 0)`
    })
  }

  return (
    <div
      className={cn(
        'group/spot relative isolate overflow-hidden rounded-card border border-hairline bg-surface-1',
        className,
      )}
      style={{ '--spot': spotlightColor, ...style } as CSSProperties}
      onPointerMove={handleMove}
      {...rest}
    >
      <span
        ref={lightRef}
        aria-hidden="true"
        className="pointer-events-none absolute top-0 left-0 -z-10 rounded-full opacity-0 transition-opacity duration-500 group-hover/spot:opacity-100"
        style={{
          width: size,
          height: size,
          background: 'radial-gradient(circle, var(--spot), transparent 65%)',
        }}
      />
      {children}
    </div>
  )
}
