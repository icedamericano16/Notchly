import { useOffscreenPause } from '@/hooks/useOffscreenPause'
import { cn } from '@/lib/cn'

/**
 * Low-opacity aurora behind the hero: three soft radial gradients drifting slowly
 * (transform only, paused offscreen, still with reduced motion). Pure CSS, so it costs no
 * JavaScript and no WebGL.
 */
export function AuroraBackground({ className }: { className?: string }) {
  const ref = useOffscreenPause<HTMLDivElement>()
  return (
    <div
      ref={ref}
      aria-hidden="true"
      className={cn(
        'motion-deferred pointer-events-none absolute inset-0 -z-10 overflow-hidden',
        className,
      )}
    >
      {/* Sized in viewport units, not % of the hero, so they never move when the hero's
          height settles (no layout shift). */}
      <div className="absolute -top-[18vw] left-[5%] h-[52vw] max-h-[44rem] w-[60%] animate-drift-a rounded-full bg-[radial-gradient(closest-side,rgb(59_123_255/0.32),transparent)] will-change-transform" />
      <div className="absolute -top-[16vw] right-0 h-[48vw] max-h-[40rem] w-[55%] animate-drift-b rounded-full bg-[radial-gradient(closest-side,rgb(139_92_246/0.28),transparent)] will-change-transform" />
      <div className="absolute top-[8vw] left-[30%] h-[40vw] max-h-[34rem] w-[45%] animate-drift-c rounded-full bg-[radial-gradient(closest-side,rgb(91_108_255/0.2),transparent)] will-change-transform" />
      {/* Fade to the canvas at the bottom (a gradient layer, cheaper than a mask). */}
      <div className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-b from-transparent to-canvas" />
      {/* Fine top hairline glow where the notch "sits". */}
      <div className="absolute top-0 left-1/2 h-px w-[60%] -translate-x-1/2 bg-gradient-to-r from-transparent via-white/25 to-transparent" />
    </div>
  )
}
