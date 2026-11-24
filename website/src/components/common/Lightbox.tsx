import { ChevronLeft, ChevronRight } from 'lucide-react'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import type { Media } from '@/content/media'
import { MediaImage } from './MediaImage'
import { VideoPlayer } from './VideoPlayer'

interface LightboxProps {
  items: Media[]
  index: number | null
  onIndexChange: (index: number | null) => void
}

/** Full-size media in a dialog; ←/→ (and the buttons) move between items. */
export function Lightbox({ items, index, onIndexChange }: LightboxProps) {
  const item = index === null ? null : items[index]
  const go = (delta: number) => {
    if (index === null) return
    onIndexChange((index + delta + items.length) % items.length)
  }

  return (
    <Dialog open={item != null} onOpenChange={(open) => !open && onIndexChange(null)}>
      <DialogContent
        className="w-[min(92vw,72rem)] max-w-none gap-0 border-hairline bg-surface-1 p-2 sm:max-w-none"
        onKeyDown={(e) => {
          if (e.key === 'ArrowRight') go(1)
          if (e.key === 'ArrowLeft') go(-1)
        }}
      >
        {item && (
          <>
            <div className="overflow-hidden rounded-xl bg-canvas">
              {item.kind === 'image' ? (
                <MediaImage media={item} priority />
              ) : (
                <VideoPlayer media={item} />
              )}
            </div>
            <div className="flex items-center justify-between gap-4 px-3 pt-3 pb-1">
              <div className="min-w-0">
                <DialogTitle className="text-base font-medium">{item.caption}</DialogTitle>
                <DialogDescription className="truncate text-sm text-fg-muted">
                  {item.alt}
                </DialogDescription>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <span className="font-mono text-xs text-fg-muted">
                  {(index ?? 0) + 1} / {items.length}
                </span>
                <button
                  type="button"
                  onClick={() => go(-1)}
                  aria-label="Previous"
                  className="flex size-9 items-center justify-center rounded-full border border-hairline hover:bg-surface-3"
                >
                  <ChevronLeft className="size-4" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  onClick={() => go(1)}
                  aria-label="Next"
                  className="flex size-9 items-center justify-center rounded-full border border-hairline hover:bg-surface-3"
                >
                  <ChevronRight className="size-4" aria-hidden="true" />
                </button>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
