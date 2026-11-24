import { Helmet } from 'react-helmet-async'
import { useLocation } from 'react-router'
import { routeMeta, site, type RouteMetaKey } from '@/content/site'

interface SeoProps {
  page: RouteMetaKey
  /** Overrides the description from routeMeta. */
  description?: string
  noindex?: boolean
}

/** Per-route title, description, canonical and social tags. Full pass in step 7. */
export function Seo({ page, description, noindex = false }: SeoProps) {
  const { pathname } = useLocation()
  const meta = routeMeta[page]
  const title = page === 'home' ? meta.title : `${meta.title} · ${site.name}`
  const desc = description ?? meta.description
  const url = `${site.url}${pathname === '/' ? '' : pathname}`

  return (
    <Helmet>
      <title>{title}</title>
      <meta name="description" content={desc} />
      <link rel="canonical" href={url} />
      {noindex && <meta name="robots" content="noindex" />}
      <meta property="og:type" content="website" />
      <meta property="og:site_name" content={site.name} />
      <meta property="og:title" content={title} />
      <meta property="og:description" content={desc} />
      <meta property="og:url" content={url} />
      <meta name="twitter:card" content="summary_large_image" />
      <meta name="twitter:title" content={title} />
      <meta name="twitter:description" content={desc} />
    </Helmet>
  )
}
