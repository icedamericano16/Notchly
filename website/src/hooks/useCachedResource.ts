import { useEffect, useState } from 'react'
import { safeSession } from '@/lib/utils'

export type Resource<T> =
  | { status: 'loading'; data: null; error: null }
  | { status: 'success'; data: T; error: null }
  | { status: 'error'; data: null; error: Error }

interface CacheEntry<T> {
  at: number
  data: T
}

/**
 * Fetches once per session: the result is cached in sessionStorage for `ttlMs`, so moving
 * between pages doesn't spend GitHub's 60-requests-an-hour budget.
 */
export function useCachedResource<T>(
  key: string,
  fetcher: (signal: AbortSignal) => Promise<T>,
  ttlMs = 10 * 60 * 1000,
): Resource<T> {
  const [state, setState] = useState<Resource<T>>(() => {
    const hit = safeSession.get<CacheEntry<T>>(key)
    return hit && Date.now() - hit.at < ttlMs
      ? { status: 'success', data: hit.data, error: null }
      : { status: 'loading', data: null, error: null }
  })
  const needsFetch = state.status === 'loading'

  useEffect(() => {
    if (!needsFetch) return
    const ctrl = new AbortController()
    fetcher(ctrl.signal)
      .then((data) => {
        safeSession.set(key, { at: Date.now(), data } satisfies CacheEntry<T>)
        setState({ status: 'success', data, error: null })
      })
      .catch((err: unknown) => {
        if (ctrl.signal.aborted) return
        setState({
          status: 'error',
          data: null,
          error: err instanceof Error ? err : new Error(String(err)),
        })
      })
    return () => ctrl.abort()
    // `fetcher` is expected to be a stable module-level function.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, needsFetch])

  return state
}
