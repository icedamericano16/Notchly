import { createBrowserRouter, matchRoutes } from 'react-router'
import { basename, routes } from './routes'

/**
 * Creates the browser router. Prerendered pages are hydrated, so any lazy route matching
 * the current URL is loaded first: hydration must render exactly what the server did.
 */
export async function createAppRouter() {
  const matches = matchRoutes(routes, window.location, basename) ?? []
  await Promise.all(
    matches.map(async (m) => {
      const lazy = m.route.lazy
      if (typeof lazy !== 'function') return
      const mod = await lazy()
      Object.assign(m.route, { ...mod, lazy: undefined })
    }),
  )
  return createBrowserRouter(routes, { basename })
}
