import type { IslandActivityId } from './island'
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
  activity: IslandActivityId
  /** Show the island open (the control center step). */
  expanded?: boolean
}

export const storySteps: StoryStep[] = [
  {
    id: 'music',
    eyebrow: '01 · Music',
    title: 'Music with synced lyrics',
    body: 'Anything that plays in the Windows media flyout shows up in the notch: album art, a live audio visualiser and lyrics that follow the song line by line. Click a line to jump there.',
    activity: 'music',
  },
  {
    id: 'calls',
    eyebrow: '02 · Calls & timers',
    title: 'Calls and timers, front and center',
    body: 'An incoming call takes the pill with accept and decline. Focus timers count down at a glance; meeting mode shows Teams, Zoom, Meet and Discord calls with mute and leave.',
    activity: 'call',
  },
  {
    id: 'downloads',
    eyebrow: '03 · Downloads & system',
    title: 'Downloads and system changes',
    body: 'Downloads show live progress. Volume, brightness, battery, Wi-Fi and Caps Lock changes appear briefly and merge when they repeat. Fullscreen games and video hide the notch on their own.',
    activity: 'download',
  },
  {
    id: 'notifications',
    eyebrow: '04 · Notifications',
    title: 'Notifications that stay out of your way',
    body: 'Windows notifications are mirrored into the notch, so they appear where your eyes already are, and quiet hours decide what still gets through.',
    activity: 'notification',
  },
  {
    id: 'control',
    eyebrow: '05 · Control Center',
    title: 'Hover to open the control center',
    body: 'Hover the notch and it opens into a small control center: Wi-Fi, Bluetooth, camera and focus switches, brightness and volume, and pages for every tool. Move away and it closes.',
    activity: 'music',
    expanded: true,
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
