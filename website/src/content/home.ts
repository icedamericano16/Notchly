import { site } from './site'

export interface HeroContent {
  eyebrow: string
  headline: string
  /** The part of the headline drawn with the accent gradient. Must appear in `headline`. */
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
