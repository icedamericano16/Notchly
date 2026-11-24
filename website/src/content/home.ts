import type { CaptureId } from './media'
import { site } from './site'

export interface HeroContent {
  eyebrow: string
  headline: string
  /** The trailing part of the headline drawn with the accent gradient. */
  headlineAccent: string
  subheadline: string
  primaryCta: string
  secondaryCta: string
  microcopy: string
}

export const hero: HeroContent = {
  eyebrow: `Free for ${site.facts.platforms}`,
  headline: 'The Dynamic Island, finally on Windows.',
  headlineAccent: 'finally on Windows.',
  subheadline: `Free, native, about ${site.facts.exeSize}. Music, timers, calls, AI and ${site.facts.toolCount} tools, living in your notch.`,
  primaryCta: 'Download for Windows',
  secondaryCta: 'View on GitHub',
  microcopy: 'Windows 10 & 11 · Free · Runs locally',
}

export const trustFacts: string[] = [
  'Native Rust',
  'No Electron',
  'GPU-rendered',
  site.facts.exeSize,
  `${site.facts.unitTests} unit tests`,
  'Local-first',
  'Open on GitHub',
]

/* ─── Scroll story ─────────────────────────────────────────────────────── */

export interface StoryStep {
  id: string
  eyebrow: string
  title: string
  body: string
  /** Real captures of the app shown on the stage for this step, in order. */
  captures: CaptureId[]
}

export const storySteps: StoryStep[] = [
  {
    id: 'music',
    eyebrow: '01 · Music',
    title: 'Music with synced lyrics',
    body: 'Anything that plays in the Windows media flyout shows up in the notch: album art and a live visualiser when it is collapsed; open it for progress, volume, the player it comes from, and lyrics that follow the song.',
    captures: ['pillMusic', 'music'],
  },
  {
    id: 'timers',
    eyebrow: '02 · Timers & calls',
    title: 'Timers and calls, front and center',
    body: 'A running timer counts down right in the notch; open it for focus, short and long breaks and one-tap presets. During Teams, Zoom, Meet or Discord calls, meeting mode shows the call with mute and leave.',
    captures: ['pillTimer', 'timer'],
  },
  {
    id: 'downloads',
    eyebrow: '03 · Downloads & system',
    title: 'Downloads and system changes',
    body: 'Downloads are followed as they grow and pop up when they finish, with Open, Show and Shelf. Brightness, Caps Lock, battery, Wi-Fi and volume changes appear briefly and merge when they repeat.',
    captures: ['popupDownload', 'popupBrightness', 'popupCapslock', 'popupCharging'],
  },
  {
    id: 'notifications',
    eyebrow: '04 · Notifications',
    title: 'Notifications that stay out of your way',
    body: 'Windows notifications are mirrored into the notch, over whatever it is showing, so they appear where your eyes already are; quiet hours decide what still gets through.',
    captures: ['timerNotification'],
  },
  {
    id: 'control',
    eyebrow: '05 · Control Center',
    title: 'Hover to open the Control Center',
    body: 'Hover the notch and it opens into Control Center: Wi-Fi, Bluetooth, airplane mode, focus, microphone, camera and dark mode, brightness and volume, battery, and quick actions. Move away and it closes.',
    captures: ['controlCenter'],
  },
]

/* ─── Performance ──────────────────────────────────────────────────────── */

export interface Stat {
  value: number
  decimals?: number
  suffix: string
  label: string
}

export const stats: Stat[] = [
  { value: site.facts.exeSizeMb, decimals: 1, suffix: ' MB', label: 'One native exe' },
  { value: site.facts.unitTests, suffix: '', label: 'Unit tests' },
  { value: 0, suffix: '', label: 'Web runtimes' },
  { value: 0, suffix: '%', label: 'CPU when idle' },
]

export const whyRust = {
  title: 'Why native Rust',
  paragraphs: [
    'Notchly is a single Rust executable that draws with Direct2D and DirectWrite on a DirectComposition swap chain. There is no browser engine inside, so it starts instantly, uses almost no memory and sits at 0% CPU while nothing is happening.',
    "Animations are spring physics running at your display's refresh rate, on the GPU. That's how a notch can feel alive all day without costing you battery.",
  ],
}

/* ─── Final CTA ────────────────────────────────────────────────────────── */

export const finalCta = {
  title: 'Put a living notch on your screen.',
  body: `Free for ${site.facts.platforms}. About ${site.facts.exeSize}. No account, no telemetry.`,
}
