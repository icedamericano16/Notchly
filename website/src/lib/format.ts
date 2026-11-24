/** 3533997 → "3.4 MB" */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const kb = bytes / 1024
  if (kb < 1024) return `${kb.toFixed(0)} KB`
  return `${(kb / 1024).toFixed(1)} MB`
}

/** ISO date → "Nov 24, 2026" */
export function formatDate(iso: string | null): string {
  if (!iso) return ''
  return new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric', year: 'numeric' }).format(
    new Date(iso),
  )
}

/** 1234 → "1,234" */
export function formatNumber(n: number): string {
  return new Intl.NumberFormat('en').format(n)
}

/** Prefix a public-folder path with Vite's base, so it works under /Notchly/ on Pages. */
export function asset(path: string): string {
  return `${import.meta.env.BASE_URL}${path.replace(/^\//, '')}`
}
