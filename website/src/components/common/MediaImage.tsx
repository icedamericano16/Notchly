import type { ImageMedia } from '@/content/media'
import { cn } from '@/lib/cn'
import { asset } from '@/lib/format'

interface MediaImageProps {
  media: ImageMedia
  className?: string
  /** Above the fold: load eagerly with high priority. */
  priority?: boolean
  sizes?: string
}

/** <picture> with AVIF → WebP → fallback, explicit size (no layout shift), lazy by default. */
export function MediaImage({ media, className, priority = false, sizes }: MediaImageProps) {
  const { src, width, height, alt } = media
  return (
    <picture>
      {src.avif && <source type="image/avif" srcSet={asset(src.avif)} sizes={sizes} />}
      {src.webp && <source type="image/webp" srcSet={asset(src.webp)} sizes={sizes} />}
      <img
        src={asset(src.fallback)}
        alt={alt}
        width={width}
        height={height}
        loading={priority ? 'eager' : 'lazy'}
        decoding="async"
        fetchPriority={priority ? 'high' : 'auto'}
        className={cn('block h-auto w-full', className)}
      />
    </picture>
  )
}
