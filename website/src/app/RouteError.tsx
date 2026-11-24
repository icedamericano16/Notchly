import { isRouteErrorResponse, Link, useRouteError } from 'react-router'
import { Button } from '@/components/ui/button'

/** Shown when a route throws or a lazy chunk fails to load (e.g. after a redeploy). */
export function RouteError() {
  const error = useRouteError()
  const detail = isRouteErrorResponse(error)
    ? `${error.status} ${error.statusText}`
    : error instanceof Error
      ? error.message
      : 'Unknown error'

  return (
    <main className="container-page flex min-h-dvh flex-col items-start justify-center gap-6">
      <h1 className="text-4xl text-display sm:text-5xl">Something went wrong.</h1>
      <p className="max-w-lg text-fg-muted">
        The page failed to load. A reload usually fixes it.
        <span className="mt-2 block font-mono text-xs text-fg-muted">{detail}</span>
      </p>
      <div className="flex gap-3">
        <Button className="rounded-full" onClick={() => window.location.reload()}>
          Reload
        </Button>
        <Button asChild variant="outline" className="rounded-full">
          <Link to="/">Home</Link>
        </Button>
      </div>
    </main>
  )
}
