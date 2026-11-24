import { site } from '@/content/site'

const API = 'https://api.github.com'

export class GitHubError extends Error {
  readonly status: number
  /** True when GitHub's unauthenticated limit (60 requests/hour per IP) was hit. */
  readonly rateLimited: boolean

  constructor(status: number, rateLimited: boolean) {
    super(rateLimited ? 'GitHub API rate limit reached' : `GitHub API error ${status}`)
    this.name = 'GitHubError'
    this.status = status
    this.rateLimited = rateLimited
  }
}

async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(`${API}${path}`, {
    headers: { Accept: 'application/vnd.github+json' },
    signal,
  })
  if (!res.ok) {
    const rateLimited =
      (res.status === 403 || res.status === 429) && res.headers.get('x-ratelimit-remaining') === '0'
    throw new GitHubError(res.status, rateLimited)
  }
  return (await res.json()) as T
}

/* ─── Repository ──────────────────────────────────────────────────────────── */

interface GitHubRepoResponse {
  stargazers_count: number
  html_url: string
}

export interface RepoInfo {
  stars: number
  url: string
}

/* ─── Releases ────────────────────────────────────────────────────────────── */

interface GitHubAssetResponse {
  name: string
  size: number
  download_count: number
  browser_download_url: string
  content_type: string
  /** "sha256:…", provided by GitHub for release assets. */
  digest?: string | null
}

interface GitHubReleaseResponse {
  tag_name: string
  name: string | null
  body: string | null
  html_url: string
  published_at: string | null
  draft: boolean
  prerelease: boolean
  assets: GitHubAssetResponse[]
}

export interface ReleaseAsset {
  name: string
  size: number
  downloadCount: number
  url: string
  contentType: string
  /** Hex SHA-256 when GitHub provides one. */
  sha256: string | null
}

export interface Release {
  tag: string
  name: string
  /** "0.3.0", parsed from the tag, then the release name, then the asset names. */
  version: string | null
  body: string
  url: string
  publishedAt: string | null
  prerelease: boolean
  assets: ReleaseAsset[]
}

const VERSION_RE = /(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?)/

export function parseVersion(...candidates: (string | null | undefined)[]): string | null {
  for (const c of candidates) {
    const match = c?.match(VERSION_RE)
    if (match?.[1]) return match[1]
  }
  return null
}

function toRelease(r: GitHubReleaseResponse): Release {
  const assets = r.assets.map<ReleaseAsset>((a) => ({
    name: a.name,
    size: a.size,
    downloadCount: a.download_count,
    url: a.browser_download_url,
    contentType: a.content_type,
    sha256: a.digest?.startsWith('sha256:') ? a.digest.slice(7) : null,
  }))
  return {
    tag: r.tag_name,
    name: r.name || r.tag_name,
    version: parseVersion(r.tag_name, r.name, ...assets.map((a) => a.name)),
    body: r.body ?? '',
    url: r.html_url,
    publishedAt: r.published_at,
    prerelease: r.prerelease,
    assets,
  }
}

const repoPath = `/repos/${site.repo.owner}/${site.repo.name}`

/** The newest non-draft, non-prerelease release. 404 means there's no release yet. */
export async function fetchLatestRelease(signal?: AbortSignal): Promise<Release> {
  return toRelease(await getJson<GitHubReleaseResponse>(`${repoPath}/releases/latest`, signal))
}

/** Up to 30 published releases, newest first, for the changelog. */
export async function fetchReleases(signal?: AbortSignal): Promise<Release[]> {
  const list = await getJson<GitHubReleaseResponse[]>(`${repoPath}/releases?per_page=30`, signal)
  return list.filter((r) => !r.draft).map(toRelease)
}

/* ─── Repository (stars) ──────────────────────────────────────────────────── */

export async function fetchRepo(signal?: AbortSignal): Promise<RepoInfo> {
  const r = await getJson<GitHubRepoResponse>(`/repos/${site.repo.owner}/${site.repo.name}`, signal)
  return { stars: r.stargazers_count, url: r.html_url }
}
