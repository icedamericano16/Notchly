/** True for absolute http(s) URLs, which open in a new tab. */
export function isExternal(href: string): boolean {
  return /^https?:\/\//.test(href)
}

/** "1234" → "1.2k", for compact counters such as GitHub stars. */
export function formatCompact(n: number): string {
  return new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 }).format(n)
}

/** sessionStorage that never throws (private mode, blocked storage, SSR). */
export const safeSession = {
  get<T>(key: string): T | null {
    try {
      const raw = sessionStorage.getItem(key)
      return raw ? (JSON.parse(raw) as T) : null
    } catch {
      return null
    }
  },
  set(key: string, value: unknown): void {
    try {
      sessionStorage.setItem(key, JSON.stringify(value))
    } catch {
      /* storage unavailable: caching is best-effort */
    }
  },
}
