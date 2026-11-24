/**
 * Every screenshot and video on the site. Components only read this file, so swapping in
 * real captures (recorded with `notchly --demo`) means editing paths here, not components.
 *
 * Paths are relative to /public. To replace a placeholder:
 *   1. Export the capture as AVIF + WebP (images) or WebM + MP4 (videos) into public/media/.
 *   2. Point `src` / `sources` at the files, set the real width/height, delete `placeholder`.
 * See website/README.md › "Swapping media".
 */

interface MediaBase {
  id: string
  alt: string
  caption: string
  width: number
  height: number
  /** True until a real capture replaces the generated placeholder. */
  placeholder?: boolean
}

export interface ImageMedia extends MediaBase {
  kind: 'image'
  src: { avif?: string; webp?: string; fallback: string }
}

export interface VideoMedia extends MediaBase {
  kind: 'video'
  poster: string
  sources: { webm?: string; mp4?: string }
}

export type Media = ImageMedia | VideoMedia

const ph = (id: string) => `media/placeholders/${id}.svg`

const image = (
  id: string,
  caption: string,
  alt: string,
  width = 1200,
  height = 800,
): ImageMedia => ({
  kind: 'image',
  id,
  caption,
  alt,
  width,
  height,
  placeholder: true,
  src: { fallback: ph(id) },
})

/** The demo video in the hero's desktop frame. */
export const heroVideo: VideoMedia = {
  kind: 'video',
  id: 'hero-demo',
  caption: 'Notchly cycling through live activities on a Windows 11 desktop',
  alt: 'Notchly on a Windows desktop: the notch shows music, a timer, a call and a download, then opens into the control center.',
  width: 1920,
  height: 1080,
  placeholder: true,
  poster: ph('hero-demo'),
  sources: { webm: 'media/hero-demo.webm', mp4: 'media/hero-demo.mp4' },
}

/** One screenshot per install step on /download. */
export const installShots: ImageMedia[] = [
  image('install-1', 'Step 1: run the installer', 'The Notchly installer window.'),
  image(
    'install-2',
    'Step 2: More info → Run anyway',
    'Windows SmartScreen with the More info link and the Run anyway button.',
  ),
  image(
    'install-3',
    'Step 3: hover the notch',
    'The notch at the top of the screen, opening on hover.',
  ),
]

/** Social preview (rendered from scripts/og.html). */
export const ogImage = 'og.png'
