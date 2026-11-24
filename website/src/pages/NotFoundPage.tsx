import { Link } from 'react-router'
import { Seo } from '@/components/common/Seo'
import { Button } from '@/components/ui/button'

/** Styled 404. The island with a "Page not found" activity is added in step 6. */
export default function NotFoundPage() {
  return (
    <>
      <Seo page="notFound" noindex />
      <section className="container-page flex min-h-[70dvh] flex-col items-center justify-center pt-24 text-center">
        <p className="font-mono text-sm text-fg-muted">404</p>
        <h1 className="mt-3 text-5xl text-display sm:text-6xl">Page not found.</h1>
        <p className="mt-4 max-w-md text-fg-muted">
          This page doesn't exist, or it moved. The notch is still where you left it.
        </p>
        <Button asChild size="lg" className="mt-8 h-11 rounded-full px-5">
          <Link to="/">Back home</Link>
        </Button>
      </section>
    </>
  )
}
