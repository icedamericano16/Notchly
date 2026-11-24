import { Expand } from 'lucide-react'
import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { useMemo, useState } from 'react'
import { Lightbox } from '@/components/common/Lightbox'
import { MediaImage } from '@/components/common/MediaImage'
import { SectionHeading } from '@/components/common/SectionHeading'
import { gallery, mediaCategories, type MediaCategory } from '@/content/media'
import { cn } from '@/lib/cn'
import { asset } from '@/lib/format'
import { islandSpring } from '@/lib/motion'

type Filter = MediaCategory | 'all'

export function Gallery() {
  const [filter, setFilter] = useState<Filter>('all')
  const [open, setOpen] = useState<number | null>(null)
  const items = useMemo(
    () => (filter === 'all' ? gallery : gallery.filter((g) => g.category === filter)),
    [filter],
  )
  const filters: { id: Filter; label: string }[] = [{ id: 'all', label: 'All' }, ...mediaCategories]

  return (
    <section aria-labelledby="gallery-title" className="container-page pt-32 sm:pt-40">
      <SectionHeading
        id="gallery-title"
        eyebrow="Gallery"
        title="See it on a real desktop."
        description="Screenshots and clips of Notchly at work. Click any of them to look closer."
      />

      <div
        role="group"
        aria-label="Filter gallery"
        className="mt-10 flex flex-wrap justify-center gap-2"
      >
        {filters.map((f) => {
          const active = f.id === filter
          return (
            <button
              key={f.id}
              type="button"
              aria-pressed={active}
              onClick={() => setFilter(f.id)}
              className={cn(
                'relative rounded-full px-4 py-2 text-sm transition-colors',
                active ? 'text-canvas' : 'text-fg-muted hover:text-fg',
              )}
            >
              {active && (
                <m.span
                  layoutId="gallery-filter"
                  transition={islandSpring}
                  className="absolute inset-0 rounded-full bg-fg"
                />
              )}
              <span className="relative">{f.label}</span>
            </button>
          )
        })}
      </div>

      <m.ul layout className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <AnimatePresence mode="popLayout" initial={false}>
          {items.map((item, i) => (
            <m.li
              key={item.id}
              layout
              initial={{ opacity: 0, scale: 0.96 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.96 }}
              transition={islandSpring}
            >
              <button
                type="button"
                onClick={() => setOpen(i)}
                aria-label={`Open ${item.caption}`}
                className="group relative block w-full overflow-hidden rounded-card border border-hairline bg-surface-1 text-left"
              >
                {item.kind === 'image' ? (
                  <MediaImage
                    media={item}
                    sizes="(min-width: 1024px) 33vw, (min-width: 640px) 50vw, 100vw"
                    className="transition-transform duration-700 ease-out-expo group-hover:scale-[1.03]"
                  />
                ) : (
                  <img
                    src={asset(item.poster)}
                    alt={item.alt}
                    width={item.width}
                    height={item.height}
                    loading="lazy"
                    className="block h-auto w-full"
                  />
                )}
                <span className="absolute inset-x-0 bottom-0 flex items-center justify-between bg-gradient-to-t from-canvas/90 to-transparent p-4 pt-10">
                  <span className="text-sm font-medium">{item.caption}</span>
                  <Expand
                    className="size-4 text-fg-muted opacity-0 transition-opacity group-hover:opacity-100"
                    aria-hidden="true"
                  />
                </span>
              </button>
            </m.li>
          ))}
        </AnimatePresence>
      </m.ul>

      <Lightbox items={items} index={open} onIndexChange={setOpen} />
    </section>
  )
}
