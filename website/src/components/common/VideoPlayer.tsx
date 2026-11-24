import { useEffect, useRef } from 'react'
import type { VideoMedia } from '@/content/media'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/cn'
import { asset } from '@/lib/format'

interface VideoPlayerProps {
  media: VideoMedia
  className?: string
}

/**
 * Muted, looping, inline video that downloads nothing until it's near the viewport
 * (preload="none" + poster), plays only while visible and pauses offscreen. With reduced
 * motion it shows the poster and doesn't autoplay. Placeholders render the poster image.
 */
export function VideoPlayer({ media, className }: VideoPlayerProps) {
  const ref = useRef<HTMLVideoElement>(null)
  const reduced = useReducedMotion()

  useEffect(() => {
    const video = ref.current
    if (!video || reduced) return
    const io = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) void video.play().catch(() => undefined)
        else video.pause()
      },
      { threshold: 0.25 },
    )
    io.observe(video)
    return () => io.disconnect()
  }, [reduced])

  if (media.placeholder) {
    return (
      <img
        src={asset(media.poster)}
        alt={media.alt}
        width={media.width}
        height={media.height}
        loading="lazy"
        decoding="async"
        className={cn('block h-auto w-full', className)}
      />
    )
  }

  return (
    <video
      ref={ref}
      className={cn('block h-auto w-full', className)}
      width={media.width}
      height={media.height}
      poster={asset(media.poster)}
      preload="none"
      muted
      loop
      playsInline
      aria-label={media.alt}
    >
      {media.sources.webm && <source src={asset(media.sources.webm)} type="video/webm" />}
      {media.sources.mp4 && <source src={asset(media.sources.mp4)} type="video/mp4" />}
    </video>
  )
}
