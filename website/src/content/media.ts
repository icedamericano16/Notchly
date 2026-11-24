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
 * Real captures of Notchly 0.3.0, the notch cut out of its background. They were taken on a
 * 1920-px-wide screen at 125% scaling, from a clean test profile (no personal data): pages
 * opened with `notchly --page <name>`, pop-ups and the timer from `notchly --demo`, music
 * from a near-silent demo track playing in Edge, the download from the real installer
 * being written to a watched folder. Nothing in them is mocked up; the Control Center's
 * Wi-Fi network name is blanked.
 *
 * Every value the site shows from the app comes from these images, not from site code.
 */
export interface AppCapture {
  id: string
  alt: string
  webp: string
  png: string
  /** Pixel size of the capture (physical pixels). */
  width: number
  height: number
}

/** Width of the screen the captures were taken on, in physical pixels. */
export const captureScreenWidth = 1920
/** Windows display scaling of that screen: physical px / CSS px. */
export const captureScale = 1.25

const capture = (id: string, alt: string, width: number, height: number): AppCapture => ({
  id,
  alt,
  webp: `media/app/${id}.webp`,
  png: `media/app/${id}.png`,
  width,
  height,
})

/** Every real capture, by id. */
export const captures = {
  pillIdle: capture(
    'pill-idle',
    'The collapsed notch at rest: the time and the battery level.',
    261,
    54,
  ),
  pillMusic: capture(
    'pill-music',
    'The collapsed notch while music plays: album art on the left, the visualiser on the right.',
    317,
    40,
  ),
  pillTimer: capture('pill-timer', 'The collapsed notch with a timer running.', 317, 40),
  music: capture(
    'music',
    'The music page: Demo track by Notchly, playing in Edge, at 0:07 with 0:23 left, with lyrics, volume and play/pause.',
    603,
    188,
  ),
  timer: capture(
    'timer',
    'The timer page: Focus 25 minutes, with Short 5m and Long 15m, and presets from 5 to 60 minutes.',
    653,
    255,
  ),
  timerNotification: capture(
    'timer-notification',
    'A mirrored notification ("Demo notification") over a running timer at 0:50.',
    655,
    257,
  ),
  popupDownload: capture(
    'popup-download',
    'Download complete: Notchly-0.3.0-setup.exe, with Open, Show and Shelf buttons.',
    481,
    105,
  ),
  popupBrightness: capture('popup-brightness', 'The brightness pop-up at 70%.', 420, 76),
  popupCapslock: capture('popup-capslock', 'The Caps Lock pop-up.', 417, 75),
  popupCharging: capture('popup-charging', 'The charging pop-up at 64%.', 417, 75),
  controlCenter: capture(
    'control-center',
    'Control Center: Wi-Fi, Bluetooth off, Airplane off, Focus off, Microphone, Camera blocked, Dark mode on, brightness and volume sliders, battery at 80% and charging, and quick actions.',
    703,
    360,
  ),
  sysmon: capture(
    'sysmon',
    'System monitor: CPU 10%, memory 68%, GPU 16%, disk 0 B/s, network 421 B/s.',
    603,
    188,
  ),
  shelf: capture(
    'shelf',
    'The shelf holding three files: Notchly-0.3.0-setup.exe (3.4 MB), README.md (5 KB) and notchly-logo.svg.',
    727,
    215,
  ),
  ask: capture(
    'ask',
    'Ask: "Ask anything, or type maths like 12% of 340", local first with Ollama, with Clipboard, Screen text, Current song and Shelf file as context.',
    727,
    215,
  ),
  todo: capture(
    'todo',
    'The to-do list: Call mom at 6:00 PM, Standup tomorrow at 9:00 AM, Dentist on Nov 27 at 2:30 PM.',
    571,
    322,
  ),
  water: capture('water', 'The water tracker: 0 of 2000 ml, add 250 ml.', 603, 189),
} satisfies Record<string, AppCapture>

export type CaptureId = keyof typeof captures

/** The hero's desktop frame: the app's pages, one after another. */
export const heroFrameCaptures: AppCapture[] = [
  captures.music,
  captures.timer,
  captures.todo,
  captures.sysmon,
  captures.shelf,
  captures.ask,
  captures.water,
]

/** The hero's notch: the collapsed states and pop-ups, at real size. */
export const heroNotchCaptures: AppCapture[] = [
  captures.pillMusic,
  captures.pillTimer,
  captures.popupDownload,
  captures.popupCapslock,
  captures.popupBrightness,
  captures.popupCharging,
  captures.pillIdle,
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
export type InstallShot = ImageMedia | { kind: 'captures'; ids: CaptureId[] }

export const installShots: InstallShot[] = [
  image('install-1', 'Step 1: run the installer', 'The Notchly installer window.'),
  image(
    'install-2',
    'Step 2: More info → Run anyway',
    'Windows SmartScreen with the More info link and the Run anyway button.',
  ),
  // Step 3: the real notch, opened: hovering it shows the Control Center.
  { kind: 'captures', ids: ['controlCenter'] },
]

/** Social preview (rendered from scripts/og.html). */
export const ogImage = 'og.png'
