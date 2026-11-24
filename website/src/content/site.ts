/**
 * Site-wide facts and navigation. Every number and claim shown on the site comes from a
 * typed content file like this one, so components stay content-agnostic.
 */

export interface NavLinkItem {
  label: string
  /** An internal route ("/features") or an absolute URL. */
  href: string
  external?: boolean
}

export interface FooterColumn {
  title: string
  links: NavLinkItem[]
}

const repoOwner = 'icedamericano16'
const repoName = 'Notchly'
const repoUrl = `https://github.com/${repoOwner}/${repoName}`

export const site = {
  name: 'Notchly',
  /** Canonical origin, used for absolute OG / canonical URLs. Change when you pick a domain. */
  url: 'https://icedamericano16.github.io/Notchly',
  tagline: 'A living Dynamic Island for Windows.',
  description:
    "A living Dynamic Island for Windows. It turns the top-center of your screen into a true-black notch that shows what's happening and opens into a small control center on hover.",

  author: {
    /** Handle shown where the project is credited ("by Ghost"). */
    handle: 'Ghost',
    /** Full name for the footer credit. */
    name: 'Khagendra Oli',
  },

  repo: {
    owner: repoOwner,
    name: repoName,
    url: repoUrl,
    releasesUrl: `${repoUrl}/releases`,
    issuesUrl: `${repoUrl}/issues`,
    licenseUrl: `${repoUrl}/blob/main/LICENSE`,
    noticesUrl: `${repoUrl}/blob/main/THIRD_PARTY_NOTICES.md`,
    sdkUrl: `${repoUrl}/tree/main/sdk`,
  },

  /** Facts quoted across the site. Update here when they change. */
  facts: {
    exeSize: '3.7 MB',
    exeSizeMb: 3.7,
    unitTests: 99,
    toolCount: '40+',
    platforms: 'Windows 10 & 11',
    price: 'Free',
  },

  disclaimer:
    "Notchly is an independent project and is not affiliated with Apple Inc. 'Dynamic Island' is used descriptively.",
} as const

export const primaryNav: NavLinkItem[] = [
  { label: 'Features', href: '/features' },
  { label: 'Download', href: '/download' },
  { label: 'Changelog', href: '/changelog' },
  { label: 'GitHub', href: repoUrl, external: true },
]

export const footerColumns: FooterColumn[] = [
  {
    title: 'Product',
    links: [
      { label: 'Features', href: '/features' },
      { label: 'Download', href: '/download' },
      { label: 'Changelog', href: '/changelog' },
      { label: 'Privacy', href: '/privacy' },
    ],
  },
  {
    title: 'Project',
    links: [
      { label: 'GitHub', href: repoUrl, external: true },
      { label: 'Releases', href: `${repoUrl}/releases`, external: true },
      { label: 'Report an issue', href: `${repoUrl}/issues`, external: true },
      { label: 'Developer SDKs', href: `${repoUrl}/tree/main/sdk`, external: true },
    ],
  },
  {
    title: 'Legal',
    links: [
      { label: 'License', href: `${repoUrl}/blob/main/LICENSE`, external: true },
      {
        label: 'Third-party notices',
        href: `${repoUrl}/blob/main/THIRD_PARTY_NOTICES.md`,
        external: true,
      },
      { label: 'Privacy policy', href: '/privacy' },
    ],
  },
]

/** Per-route SEO copy. Titles are suffixed with " · Notchly" except on the home page. */
export const routeMeta = {
  home: {
    title: 'Notchly — The Dynamic Island, finally on Windows',
    description:
      'Free, native, about 3.7 MB. Music, timers, calls, AI and 40+ tools, living in a true-black notch at the top of your screen. For Windows 10 & 11.',
  },
  features: {
    title: 'Features',
    description:
      'Every Notchly feature: Live Activities, smart tools, productivity, files & clipboard, system monitoring, themes and a developer API.',
  },
  download: {
    title: 'Download',
    description:
      'Download Notchly for Windows 10 & 11. Install guide, system requirements, build-from-source steps and checksums.',
  },
  changelog: {
    title: 'Changelog',
    description: 'Release notes for every Notchly version, straight from GitHub Releases.',
  },
  privacy: {
    title: 'Privacy',
    description:
      'Everything in Notchly runs locally. Every network feature is listed, and every one can be turned off.',
  },
  notFound: {
    title: 'Page not found',
    description: 'This page does not exist.',
  },
} as const

export type RouteMetaKey = keyof typeof routeMeta
