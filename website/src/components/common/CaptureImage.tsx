import { captureScale, type AppCapture } from '@/content/media'
import { cn } from '@/lib/cn'
import { asset } from '@/lib/format'

interface CaptureImageProps {
  capture: AppCapture
  className?: string
  /**
   * "real": the size it has on screen (capture pixels ÷ display scaling), capped at the
   * container width. "fill": the container's width.
   */
  size?: 'real' | 'fill'
  priority?: boolean
}

/** One real capture of the app (WebP with a PNG fallback), with exact dimensions. */
export function CaptureImage({
  capture,
  className,
  size = 'real',
  priority = false,
}: CaptureImageProps) {
  return (
    <picture className={cn('block', className)}>
      <source type="image/webp" srcSet={asset(capture.webp)} />
      <img
        src={asset(capture.png)}
        alt={capture.alt}
        width={capture.width}
        height={capture.height}
        loading={priority ? 'eager' : 'lazy'}
        decoding="async"
        className="block h-auto max-w-full"
        style={size === 'real' ? { width: capture.width / captureScale } : { width: '100%' }}
      />
    </picture>
  )
}
