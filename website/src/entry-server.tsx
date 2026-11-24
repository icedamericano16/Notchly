/**
 * Build-time prerendering (used by scripts/prerender.mjs, never shipped to browsers).
 * Renders one URL to HTML with React 19's `prerender`, which waits for lazy sections, so
 * the whole page, not just the hero, is in the static HTML.
 */
import { StrictMode } from 'react'
import { prerender } from 'react-dom/static'
import { createStaticHandler, createStaticRouter, StaticRouterProvider } from 'react-router'
import { Providers } from '@/app/providers'
import { basename, routes } from '@/app/routes'

export async function render(url: string): Promise<{ html: string; status: number }> {
  const handler = createStaticHandler(routes, { basename })
  const context = await handler.query(new Request(new URL(url, 'http://localhost')))
  if (context instanceof Response) throw new Error(`Unexpected redirect for ${url}`)

  const router = createStaticRouter(handler.dataRoutes, context)
  const { prelude } = await prerender(
    <StrictMode>
      <Providers>
        <StaticRouterProvider router={router} context={context} hydrate={false} />
      </Providers>
    </StrictMode>,
  )
  const html = await new Response(prelude).text()
  return { html, status: context.statusCode }
}
