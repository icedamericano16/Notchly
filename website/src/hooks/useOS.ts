import { useSyncExternalStore } from 'react'

export type OS = 'windows' | 'mac' | 'linux' | 'android' | 'ios' | 'chromeos' | 'unknown'

interface NavigatorUAData {
  platform?: string
}

function detectOS(): OS {
  if (typeof navigator === 'undefined') return 'unknown'
  const uaData = (navigator as Navigator & { userAgentData?: NavigatorUAData }).userAgentData
  const p = (uaData?.platform ?? '').toLowerCase()
  const ua = navigator.userAgent.toLowerCase()

  if (p === 'windows' || /windows nt/.test(ua)) return 'windows'
  if (p === 'android' || /android/.test(ua)) return 'android'
  if (/iphone|ipad|ipod/.test(ua)) return 'ios'
  // iPadOS reports itself as a Mac; a touch screen gives it away.
  if (p === 'macos' || /mac os x|macintosh/.test(ua)) {
    return navigator.maxTouchPoints > 1 ? 'ios' : 'mac'
  }
  if (p === 'chrome os' || /cros/.test(ua)) return 'chromeos'
  if (p === 'linux' || /linux/.test(ua)) return 'linux'
  return 'unknown'
}

export const osNames: Record<OS, string> = {
  windows: 'Windows',
  mac: 'macOS',
  linux: 'Linux',
  android: 'Android',
  ios: 'iOS',
  chromeos: 'ChromeOS',
  unknown: 'this device',
}

const noop = () => () => undefined
let cached: OS | null = null

/**
 * The visitor's OS. Prerendered HTML assumes "unknown" (treated as Windows); the real value
 * applies right after hydration. Unknown counts as "maybe Windows": we never block a
 * download on a guess.
 */
export function useOS(): { os: OS; isWindows: boolean } {
  const os = useSyncExternalStore(
    noop,
    () => (cached ??= detectOS()),
    () => 'unknown' as OS,
  )
  return { os, isWindows: os === 'windows' || os === 'unknown' }
}
