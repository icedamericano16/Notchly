import { assetPatterns, type AssetKind } from '@/content/download'
import { fetchLatestRelease, fetchReleases, type Release, type ReleaseAsset } from '@/lib/github'
import { useCachedResource, type Resource } from './useCachedResource'

export function pickAsset(assets: ReleaseAsset[], kind: AssetKind): ReleaseAsset | null {
  for (const re of assetPatterns[kind]) {
    const hit = assets.find((a) => re.test(a.name))
    if (hit) return hit
  }
  return null
}

export interface LatestRelease {
  /** loading | ready (installer found) | fallback (no release, no installer, or API error). */
  status: 'loading' | 'ready' | 'fallback'
  release: Release | null
  /** The preferred download: the installer, else the bare exe. */
  primary: ReleaseAsset | null
  msix: ReleaseAsset | null
  /** Why we're on the fallback, for the UI. */
  reason: 'none' | 'no-release' | 'rate-limited' | 'error' | 'no-installer'
}

/** The latest release and the right files, cached for the session. Never throws. */
export function useLatestRelease(): LatestRelease {
  const res = useCachedResource('notchly:release:latest', fetchLatestRelease)

  if (res.status === 'loading') {
    return { status: 'loading', release: null, primary: null, msix: null, reason: 'none' }
  }
  if (res.status === 'error') {
    const err = res.error as Error & { status?: number; rateLimited?: boolean }
    return {
      status: 'fallback',
      release: null,
      primary: null,
      msix: null,
      reason: err.rateLimited ? 'rate-limited' : err.status === 404 ? 'no-release' : 'error',
    }
  }
  const release = res.data
  const primary = pickAsset(release.assets, 'installer') ?? pickAsset(release.assets, 'portable')
  const msix = pickAsset(release.assets, 'msix')
  return {
    status: primary ? 'ready' : 'fallback',
    release,
    primary,
    msix,
    reason: primary ? 'none' : 'no-installer',
  }
}

/** All releases for the changelog. */
export function useReleases(): Resource<Release[]> {
  return useCachedResource('notchly:releases', fetchReleases)
}
