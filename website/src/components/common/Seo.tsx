import { Helmet } from 'react-helmet-async'
import { useLocation } from 'react-router'
import { ogImage } from '@/content/media'
import { routeMeta, site, type RouteMetaKey } from '@/content/site'

interface SeoProps {
  page: RouteMetaKey
  /** Overrides the description from routeMeta. */
  description?: string
  noindex?: boolean
}

/** Per-route title, description, canonical URL, Open Graph and Twitter card tags. */
export function Seo({ page, description, noindex = false }: SeoProps) {
  const { pathname } = useLocation()
  const meta = routeMeta[page]
  const title = page === 'home' ? meta.title : `${meta.title} · ${site.name}`
  const desc = description ?? meta.description
  const path = pathname.replace(/\/+$/, '')
  const url = `${site.url}${path}`

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
      <meta property="og:image" content={`${site.url}/${ogImage}`} />
      <meta property="og:image:width" content="1200" />
      <meta property="og:image:height" content="630" />
      <meta property="og:image:alt" content="Notchly: the Dynamic Island, finally on Windows." />
      <meta name="twitter:card" content="summary_large_image" />
      <meta name="twitter:title" content={title} />
      <meta name="twitter:description" content={desc} />
      <meta name="twitter:image" content={`${site.url}/${ogImage}`} />
    </Helmet>
  )
}
