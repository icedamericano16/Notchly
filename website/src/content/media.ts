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
  height = 750,
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

/**
 * Real captures of the app (taken with `notchly --demo` on a 1920-px-wide screen, the
 * notch cut out of its background). `screenWidth` is that screen's width, so a capture
 * can be drawn at its true size on a scaled-down desktop.
 */
export interface AppCapture {
  id: string
  alt: string
  webp: string
  png: string
  width: number
  height: number
}

export const captureScreenWidth = 1920

const capture = (id: string, alt: string, width: number, height: number): AppCapture => ({
  id,
  alt,
  webp: `media/app/${id}.webp`,
  png: `media/app/${id}.png`,
  width,
  height,
})

export const appCaptures: AppCapture[] = [
  capture('timer', 'The Focus timer page: 25:00, presets from 5 to 60 minutes.', 653, 255),
  capture('popup-brightness', 'The brightness pop-up at 70%.', 420, 76),
  capture('todo', 'The to-do page with its natural-language input.', 601, 376),
  capture('popup-capslock', 'The Caps Lock pop-up.', 417, 75),
  capture('water', 'The water tracker: 0 of 2000 ml, add 250 ml.', 603, 189),
  capture('popup-charging', 'The charging pop-up at 64%.', 417, 75),
  capture('music-idle', 'The music page with nothing playing.', 603, 188),
  capture('timer-notification', 'A mirrored notification over a running timer.', 655, 257),
]

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

/**
 * One picture per install step on /download: an image (placeholders until captured), or
 * real app captures shown on the neutral desktop.
 */
export type InstallShot = ImageMedia | { kind: 'captures'; ids: string[] }

export const installShots: InstallShot[] = [
  image('install-1', 'Step 1: run the installer', 'The Notchly installer window.'),
  image(
    'install-2',
    'Step 2: More info → Run anyway',
    'Windows SmartScreen with the More info link and the Run anyway button.',
  ),
  // Step 3: the real notch, opened (timer page).
  { kind: 'captures', ids: ['timer'] },
]

/** Social preview (rendered from scripts/og.html). */
export const ogImage = 'og.png'
