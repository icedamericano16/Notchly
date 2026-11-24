import { fetchRepo, type RepoInfo } from '@/lib/github'
import { useCachedResource, type Resource } from './useCachedResource'

/** Star count and URL for the navbar badge. Fails quietly: the badge just hides the count. */
export function useRepoInfo(): Resource<RepoInfo> {
  return useCachedResource('notchly:repo', fetchRepo, 30 * 60 * 1000)
}
