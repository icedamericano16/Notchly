import { useState } from 'react'
import { Link } from 'react-router'
import { Seo } from '@/components/common/Seo'
import { DynamicIsland } from '@/components/island/DynamicIsland'
import { Button } from '@/components/ui/button'
import { useMediaQuery } from '@/hooks/useMediaQuery'

/** 404 with the island showing a "Page not found" activity (hover it). */
export default function NotFoundPage() {
  const [expanded, setExpanded] = useState(false)
  const small = useMediaQuery('(max-width: 639px)')

  return (
    <>
      <Seo page="notFound" noindex />
      <section className="container-page flex min-h-[80dvh] flex-col items-center justify-center pt-28 text-center">
        <div className="relative z-10 h-10">
          <div className="absolute top-0 left-1/2 -translate-x-1/2">
            <DynamicIsland
              activity="notFound"
              expanded={expanded}
              onExpandedChange={setExpanded}
              size={small ? 'sm' : 'md'}
            />
          </div>
        </div>
        <p className="mt-16 font-mono text-sm text-fg-muted">404</p>
        <h1 className="mt-3 animate-rise-in text-5xl text-display sm:text-7xl">Page not found.</h1>
        <p className="mt-4 max-w-md text-fg-muted">
          This page doesn't exist, or it moved. The notch is still where you left it.
        </p>
        <div className="mt-8 flex gap-3">
          <Button asChild size="lg" className="h-11 rounded-full px-5">
            <Link to="/">Back home</Link>
          </Button>
          <Button
            asChild
            size="lg"
            variant="ghost"
            className="h-11 rounded-full px-5 text-fg-muted"
          >
            <Link to="/features">See features</Link>
          </Button>
        </div>
      </section>
    </>
  )
}
