import { cn } from '@/lib/cn'

/**
 * Behind the hero: a soft white light falling from the top edge (where the notch sits) and
 * a fine grid that fades out around it. Monochrome and static, so the product carries the
 * colour and the first frame costs almost nothing.
 */
export function HeroBackdrop({ className }: { className?: string }) {
  return (
    <div
      aria-hidden="true"
      className={cn('pointer-events-none absolute inset-0 -z-10 overflow-hidden', className)}
    >
      <div className="absolute inset-0 bg-grid [mask-image:radial-gradient(60%_55%_at_50%_0%,#000_20%,transparent_75%)]" />
      <div className="absolute inset-x-0 top-0 h-[42rem] bg-[radial-gradient(50%_60%_at_50%_0%,rgb(255_255_255/0.09),transparent_70%)]" />
      {/* The hairline where the screen's top edge would be. */}
      <div className="absolute top-0 left-1/2 h-px w-[60%] -translate-x-1/2 bg-gradient-to-r from-transparent via-white/25 to-transparent" />
    </div>
  )
}
