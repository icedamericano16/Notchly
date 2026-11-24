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

/** One request per key at a time, however many components ask. */
const inflight = new Map<string, Promise<unknown>>()

function readCache<T>(key: string, ttlMs: number): T | null {
  const hit = safeSession.get<CacheEntry<T>>(key)
  return hit && Date.now() - hit.at < ttlMs ? hit.data : null
}

/**
 * Fetches once per session: the result is cached in sessionStorage for `ttlMs`, so moving
 * between pages doesn't spend GitHub's 60-requests-an-hour budget. Concurrent callers share
 * one request.
 */
export function useCachedResource<T>(
  key: string,
  fetcher: (signal: AbortSignal) => Promise<T>,
  ttlMs = 10 * 60 * 1000,
): Resource<T> {
  // Always start as "loading" so prerendered HTML and the first client render match;
  // the session cache is read right after hydration.
  const [state, setState] = useState<Resource<T>>({ status: 'loading', data: null, error: null })
  const needsFetch = state.status === 'loading'

  useEffect(() => {
    if (!needsFetch) return
    let active = true
    const cached = readCache<T>(key, ttlMs)
    if (cached !== null) {
      // Deferred a microtask: keeps the effect free of synchronous setState.
      void Promise.resolve().then(() => {
        if (active) setState({ status: 'success', data: cached, error: null })
      })
      return () => {
        active = false
      }
    }
    let request = inflight.get(key) as Promise<T> | undefined
    if (!request) {
      // Shared requests aren't aborted by one unmounting caller.
      request = fetcher(new AbortController().signal).then((data) => {
        safeSession.set(key, { at: Date.now(), data } satisfies CacheEntry<T>)
        return data
      })
      inflight.set(key, request)
      void request.finally(() => inflight.delete(key)).catch(() => undefined)
    }
    request
      .then((data) => {
        if (active) setState({ status: 'success', data, error: null })
      })
      .catch((err: unknown) => {
        if (!active) return
        setState({
          status: 'error',
          data: null,
          error: err instanceof Error ? err : new Error(String(err)),
        })
      })
    return () => {
      active = false
    }
    // `fetcher` is expected to be a stable module-level function.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, needsFetch, ttlMs])

  return state
}
