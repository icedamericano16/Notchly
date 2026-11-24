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

export async function fetchRepo(signal?: AbortSignal): Promise<RepoInfo> {
  const r = await getJson<GitHubRepoResponse>(`/repos/${site.repo.owner}/${site.repo.name}`, signal)
  return { stars: r.stargazers_count, url: r.html_url }
}
