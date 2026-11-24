/**
 * The React island is only used for site states that the app itself never shows, such as
 * the 404 page's "Page not found". Everything the site shows *from the app* is a real
 * capture (see content/media.ts), never mock data.
 */

export type IslandActivityId = 'idle' | 'notFound'

export interface IslandActivityMeta {
  /** Short name ("Not found"). */
  label: string
  /** What a screen reader hears for the current state. */
  description: string
}

export const notFoundIsland = { title: 'Page not found', code: '404' } as const

export const islandMeta: Record<IslandActivityId, IslandActivityMeta> = {
  idle: { label: 'Idle', description: 'Collapsed notch.' },
  notFound: { label: 'Not found', description: 'Page not found, error 404.' },
}
