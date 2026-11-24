import { ArrowUpRight } from 'lucide-react'
import Markdown, { type Components } from 'react-markdown'
import { PageHeader } from '@/components/common/PageHeader'
import { Seo } from '@/components/common/Seo'
import { site } from '@/content/site'
import { useReleases } from '@/hooks/useLatestRelease'
import { formatBytes, formatDate } from '@/lib/format'
import type { Release } from '@/lib/github'

/** Release bodies are Markdown from GitHub. react-markdown never renders raw HTML. */
const md: Components = {
  h1: ({ children }) => (
    <h3 className="mt-8 text-xl font-semibold tracking-tightish">{children}</h3>
  ),
  h2: ({ children }) => (
    <h3 className="mt-8 text-xl font-semibold tracking-tightish">{children}</h3>
  ),
  h3: ({ children }) => <h4 className="mt-6 font-semibold">{children}</h4>,
  h4: ({ children }) => <h5 className="mt-4 font-medium">{children}</h5>,
  p: ({ children }) => <p className="mt-3 leading-relaxed text-fg-muted">{children}</p>,
  ul: ({ children }) => (
    <ul className="mt-3 list-disc space-y-1.5 pl-5 text-fg-muted marker:text-fg-faint">
      {children}
    </ul>
  ),
  ol: ({ children }) => (
    <ol className="mt-3 list-decimal space-y-1.5 pl-5 text-fg-muted">{children}</ol>
  ),
  li: ({ children }) => <li className="leading-relaxed [&>ul]:mt-1.5">{children}</li>,
  strong: ({ children }) => <strong className="font-semibold text-fg">{children}</strong>,
  code: ({ children }) => (
    <code className="rounded bg-surface-3 px-1.5 py-0.5 font-mono text-[0.85em] text-fg">
      {children}
    </code>
  ),
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-fg underline underline-offset-4"
    >
      {children}
    </a>
  ),
  hr: () => <hr className="my-6 border-hairline" />,
}

/** GitHub release bodies here start with the whole changelog's "# Changelog" title. */
function cleanBody(body: string): string {
  return body.replace(/^\s*#\s+Changelog\s*\n/i, '').trim()
}

export default function ChangelogPage() {
  const releases = useReleases()

  return (
    <>
      <Seo page="changelog" />
      <PageHeader
        eyebrow="Changelog"
        title="What's new."
        description="Release notes for every version, straight from GitHub Releases."
      />
      <div className="container-page">
        {releases.status === 'loading' && (
          <div className="flex flex-col gap-4" aria-busy="true" aria-label="Loading releases">
            {[0, 1].map((i) => (
              <div
                key={i}
                className="h-64 animate-pulse rounded-card border border-hairline bg-surface-1"
              />
            ))}
          </div>
        )}

        {releases.status === 'error' && <ChangelogFallback />}

        {releases.status === 'success' &&
          (releases.data.length === 0 ? (
            <ChangelogFallback empty />
          ) : (
            <ol className="flex flex-col gap-16">
              {releases.data.map((r) => (
                <ReleaseEntry key={r.tag + r.name} release={r} />
              ))}
            </ol>
          ))}
      </div>
    </>
  )
}

function ReleaseEntry({ release }: { release: Release }) {
  return (
    <li className="grid gap-6 lg:grid-cols-[14rem_1fr]">
      <div className="lg:sticky lg:top-28 lg:self-start">
        <p className="font-mono text-sm text-fg">
          {release.version ? `v${release.version}` : release.tag}
        </p>
        <p className="mt-1 text-sm text-fg-muted">{formatDate(release.publishedAt)}</p>
        {release.prerelease && (
          <p className="mt-2 inline-block rounded-full border border-hairline px-2 py-0.5 text-xs text-live-orange">
            Pre-release
          </p>
        )}
      </div>
      <article className="min-w-0 rounded-card border border-hairline bg-surface-1 p-6 sm:p-8">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <h2 className="text-2xl text-display">{release.name}</h2>
          <a
            href={release.url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 text-sm text-fg-muted hover:text-fg"
          >
            On GitHub <ArrowUpRight className="size-3.5" aria-hidden="true" />
            <span className="sr-only"> (opens in a new tab)</span>
          </a>
        </div>
        <div className="text-[15px]">
          <Markdown components={md}>
            {cleanBody(release.body) || 'No notes for this release.'}
          </Markdown>
        </div>
        {release.assets.length > 0 && (
          <ul className="mt-8 flex flex-wrap gap-2 border-t border-hairline pt-5">
            {release.assets.map((a) => (
              <li key={a.name}>
                <a
                  href={a.url}
                  className="inline-flex items-center gap-2 rounded-full border border-hairline px-3 py-1.5 font-mono text-xs text-fg-muted transition-colors hover:border-hairline-strong hover:text-fg"
                >
                  {a.name} <span className="text-fg-faint">{formatBytes(a.size)}</span>
                </a>
              </li>
            ))}
          </ul>
        )}
      </article>
    </li>
  )
}

function ChangelogFallback({ empty = false }: { empty?: boolean }) {
  return (
    <div className="rounded-card border border-hairline bg-surface-1 p-6">
      <p className="text-fg-muted">
        {empty
          ? 'No releases have been published yet.'
          : "GitHub couldn't be reached just now (or its hourly limit was hit)."}{' '}
        The full history is in the repository.
      </p>
      <div className="mt-4 flex flex-wrap gap-4 text-sm">
        <a
          href={site.repo.releasesUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="underline underline-offset-4"
        >
          GitHub Releases<span className="sr-only"> (opens in a new tab)</span>
        </a>
        <a
          href={`${site.repo.url}/blob/main/CHANGELOG.md`}
          target="_blank"
          rel="noopener noreferrer"
          className="underline underline-offset-4"
        >
          CHANGELOG.md<span className="sr-only"> (opens in a new tab)</span>
        </a>
      </div>
    </div>
  )
}
