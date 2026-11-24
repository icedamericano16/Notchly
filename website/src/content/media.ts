/**
 * Every screenshot and video on the site. Components only read this file, so swapping in
 * real captures (recorded with `notchly --demo`) means editing paths here, not components.
 *
 * Paths are relative to /public. To replace a placeholder:
 *   1. Export the capture as AVIF + WebP (images) or WebM + MP4 (videos) into public/media/.
 *   2. Point `src` / `sources` at the files, set the real width/height, delete `placeholder`.
 * See website/README.md › "Swapping media".
 */

export type MediaCategory = 'live' | 'tools' | 'look' | 'productivity'

export const mediaCategories: { id: MediaCategory; label: string }[] = [
  { id: 'live', label: 'Live Activities' },
  { id: 'tools', label: 'Smart tools' },
  { id: 'productivity', label: 'Productivity' },
  { id: 'look', label: 'Look & feel' },
]

interface MediaBase {
  id: string
  alt: string
  caption: string
  category: MediaCategory
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
  category: MediaCategory,
  caption: string,
  alt: string,
  width = 1600,
  height = 1000,
): ImageMedia => ({
  kind: 'image',
  id,
  category,
  caption,
  alt,
  width,
  height,
  placeholder: true,
  src: { fallback: ph(id) },
})

export const heroVideo: VideoMedia = {
  kind: 'video',
  id: 'hero-demo',
  category: 'live',
  caption: 'Notchly cycling through live activities on a Windows 11 desktop',
  alt: 'Notchly on a Windows desktop: the notch shows music, a timer, a call and a download, then opens into the control center.',
  width: 1920,
  height: 1080,
  placeholder: true,
  poster: ph('hero-demo'),
  sources: { webm: 'media/hero-demo.webm', mp4: 'media/hero-demo.mp4' },
}

export const gallery: Media[] = [
  image(
    'music-lyrics',
    'live',
    'Music with synced lyrics',
    'The expanded notch playing a song, with the current lyric line highlighted.',
  ),
  image('timer', 'live', 'Focus timer', 'A focus timer counting down in the collapsed notch.'),
  image(
    'call',
    'live',
    'Incoming call',
    'An incoming call in the notch with accept and decline buttons.',
  ),
  image(
    'downloads',
    'live',
    'Downloads',
    'A download in progress with a progress ring in the notch.',
  ),
  image(
    'notifications',
    'live',
    'Notifications',
    'A mirrored Windows notification shown in the notch.',
  ),
  image(
    'control-center',
    'productivity',
    'Control Center',
    'The control center page with Wi-Fi, Bluetooth, camera and focus switches and sliders.',
  ),
  image(
    'ask-ai',
    'tools',
    'Ask with local AI',
    'The Ask page answering a question with a local model.',
  ),
  image('ocr', 'tools', 'Smart OCR', 'Selecting a region of the screen to copy its text.'),
  image(
    'calendar',
    'productivity',
    'Calendar with Bikram Sambat',
    'The calendar page showing the next event and the Bikram Sambat date.',
  ),
  image('shelf', 'productivity', 'Shelf', 'Files dropped on the shelf, ready to drag out.'),
  image(
    'liquid-glass',
    'look',
    'Liquid Glass',
    'The notch in Liquid Glass, bending the wallpaper behind it.',
  ),
  image('themes', 'look', 'Themes', 'Theme Studio with several theme presets.'),
]

export const installShots: ImageMedia[] = [
  image(
    'install-1',
    'productivity',
    'Step 1: run the installer',
    'The Notchly installer window.',
    1200,
    800,
  ),
  image(
    'install-2',
    'productivity',
    'Step 2: More info → Run anyway',
    'Windows SmartScreen with the More info link and the Run anyway button.',
    1200,
    800,
  ),
  image(
    'install-3',
    'productivity',
    'Step 3: hover the notch',
    'The notch at the top of the screen, opening on hover.',
    1200,
    800,
  ),
]

/** Social preview (generated; see scripts/). */
export const ogImage = 'og.png'
