import { useLatestRelease } from '@/hooks/useLatestRelease'
import { site } from '@/content/site'

/**
 * SoftwareApplication structured data. A static copy lives in index.html for crawlers that
 * don't run JavaScript; this one adds the live version and file size when known.
 */
export function JsonLd() {
  const latest = useLatestRelease()
  const data = {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: site.name,
    description: site.description,
    applicationCategory: 'UtilitiesApplication',
    operatingSystem: 'Windows 10, Windows 11',
    url: site.url,
    downloadUrl: latest.primary?.url ?? site.repo.releasesUrl,
    ...(latest.release?.version ? { softwareVersion: latest.release.version } : {}),
    ...(latest.primary ? { fileSize: `${(latest.primary.size / 1048576).toFixed(1)} MB` } : {}),
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    author: { '@type': 'Person', name: site.author.name, alternateName: site.author.handle },
  }
  return (
    <script
      type="application/ld+json"
      // Static, site-authored JSON.
      dangerouslySetInnerHTML={{ __html: JSON.stringify(data) }}
    />
  )
}
