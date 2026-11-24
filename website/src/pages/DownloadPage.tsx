import { ArrowUpRight, Hash, Package } from 'lucide-react'
import { CodeBlockPlain } from '@/components/common/CodeBlockPlain'
import { CopyButton } from '@/components/common/CopyButton'
import { DesktopPreview } from '@/components/common/DesktopPreview'
import { DownloadButton } from '@/components/common/DownloadButton'
import { MediaImage } from '@/components/common/MediaImage'
import { PageHeader } from '@/components/common/PageHeader'
import { Reveal, RevealGroup, RevealItem } from '@/components/common/Reveal'
import { Seo } from '@/components/common/Seo'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { buildFromSource, installSteps, requirements, uninstallSteps } from '@/content/download'
import { captures, installShots } from '@/content/media'
import { site } from '@/content/site'
import { useLatestRelease, type LatestRelease } from '@/hooks/useLatestRelease'
import { formatBytes, formatDate, formatNumber } from '@/lib/format'

export default function DownloadPage() {
  const latest = useLatestRelease()

  return (
    <>
      <Seo page="download" />
      <PageHeader
        eyebrow="Download"
        title="Get Notchly."
        description={`Free for ${site.facts.platforms}. One installer, about ${site.facts.exeSize}, no administrator rights, no account.`}
      >
        <DownloadButton showMeta align="start" />
      </PageHeader>

      <div className="container-page flex flex-col gap-24">
        <ReleaseDetails latest={latest} />

        {/* Install guide */}
        <section aria-labelledby="install-title">
          <h2 id="install-title" className="text-3xl text-display sm:text-4xl">
            Install in three steps
          </h2>
          <RevealGroup as="ol" className="mt-8 grid gap-4 lg:grid-cols-3">
            {installSteps.map((step, i) => {
              const shot = installShots[i]
              return (
                <RevealItem
                  as="li"
                  key={step.title}
                  className="flex flex-col overflow-hidden rounded-card border border-hairline bg-surface-1"
                >
                  {shot?.kind === 'image' && (
                    <MediaImage media={shot} className="border-b border-hairline" />
                  )}
                  {shot?.kind === 'captures' && (
                    <DesktopPreview
                      className="aspect-[16/10] border-b border-hairline"
                      zoom={2.6}
                      captures={shot.ids.map((id) => captures[id])}
                      label="Real capture"
                    />
                  )}
                  <div className="flex flex-col gap-2 p-5">
                    <p className="font-mono text-xs text-fg-muted">Step {i + 1}</p>
                    <h3 className="font-medium">{step.title}</h3>
                    <p className="text-sm leading-relaxed text-fg-muted">{step.body}</p>
                  </div>
                </RevealItem>
              )
            })}
          </RevealGroup>
        </section>

        {/* Requirements */}
        <section aria-labelledby="req-title" className="grid gap-8 lg:grid-cols-[1fr_2fr]">
          <h2 id="req-title" className="text-3xl text-display sm:text-4xl">
            System requirements
          </h2>
          <Reveal>
            <dl className="divide-y divide-hairline rounded-card border border-hairline bg-surface-1">
              {requirements.map((r) => (
                <div key={r.label} className="grid gap-1 px-5 py-4 sm:grid-cols-[10rem_1fr]">
                  <dt className="text-sm text-fg-muted">{r.label}</dt>
                  <dd className="text-sm">{r.value}</dd>
                </div>
              ))}
            </dl>
          </Reveal>
        </section>

        {/* Build from source + uninstall */}
        <section aria-labelledby="more-title" className="grid gap-8 lg:grid-cols-[1fr_2fr]">
          <h2 id="more-title" className="text-3xl text-display sm:text-4xl">
            More options
          </h2>
          <Reveal>
            <Accordion
              type="multiple"
              className="rounded-card border border-hairline bg-surface-1 px-5"
            >
              <AccordionItem value="source" className="border-hairline">
                <AccordionTrigger className="py-5 text-base hover:no-underline">
                  Build from source
                </AccordionTrigger>
                <AccordionContent className="flex flex-col gap-4 pb-5 text-[15px] text-fg-muted">
                  <p>You need:</p>
                  <ul className="list-disc space-y-1 pl-5">
                    {buildFromSource.requirements.map((r) => (
                      <li key={r}>{r}</li>
                    ))}
                  </ul>
                  <CodeBlockPlain code={buildFromSource.commands} />
                  <p className="text-sm">
                    The repository's <code className="font-mono text-fg">.cargo/config.toml</code>{' '}
                    pins the linker to VS 2022 Build Tools; delete it if your Visual Studio has a
                    complete C++ workload.
                  </p>
                </AccordionContent>
              </AccordionItem>
              <AccordionItem value="uninstall" className="border-hairline">
                <AccordionTrigger className="py-5 text-base hover:no-underline">
                  Uninstall
                </AccordionTrigger>
                <AccordionContent className="pb-5 text-[15px] text-fg-muted">
                  <ol className="list-decimal space-y-2 pl-5">
                    {uninstallSteps.map((s) => (
                      <li key={s}>{s}</li>
                    ))}
                  </ol>
                </AccordionContent>
              </AccordionItem>
              <AccordionItem value="smartscreen" className="border-hairline">
                <AccordionTrigger className="py-5 text-base hover:no-underline">
                  Why does SmartScreen warn me?
                </AccordionTrigger>
                <AccordionContent className="pb-5 text-[15px] leading-relaxed text-fg-muted">
                  SmartScreen warns about any new app until it has built up download reputation with
                  Microsoft. The warning isn't a detection. Click <strong>More info</strong>, then{' '}
                  <strong>Run anyway</strong>. You can check the file against the SHA-256 checksum
                  above.
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          </Reveal>
        </section>
      </div>
    </>
  )
}

function ReleaseDetails({ latest }: { latest: LatestRelease }) {
  if (latest.status === 'loading') {
    return (
      <div
        className="h-40 animate-pulse rounded-card border border-hairline bg-surface-1"
        aria-hidden="true"
      />
    )
  }
  if (latest.status === 'fallback' || !latest.release || !latest.primary) {
    return (
      <section
        aria-labelledby="release-title"
        className="rounded-card border border-hairline bg-surface-1 p-6"
      >
        <h2 id="release-title" className="text-xl font-semibold">
          Latest builds
        </h2>
        <p className="mt-2 text-fg-muted">
          The release list couldn't be loaded here. Every build is on GitHub Releases, or you can
          build Notchly from source below.
        </p>
        <a
          href={site.repo.releasesUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="mt-4 inline-flex items-center gap-1.5 text-sm underline underline-offset-4"
        >
          Open GitHub Releases <ArrowUpRight className="size-3.5" aria-hidden="true" />
          <span className="sr-only"> (opens in a new tab)</span>
        </a>
      </section>
    )
  }

  const { release, primary, msix } = latest
  const rows = [
    { label: 'Version', value: release.version ?? release.name },
    { label: 'Released', value: formatDate(release.publishedAt) },
    { label: 'File', value: `${primary.name} · ${formatBytes(primary.size)}` },
    ...(primary.downloadCount > 0
      ? [{ label: 'Downloads', value: formatNumber(primary.downloadCount) }]
      : []),
  ]

  return (
    <section
      aria-labelledby="release-title"
      className="rounded-card border border-hairline bg-surface-1"
    >
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-hairline px-5 py-4">
        <h2 id="release-title" className="font-semibold">
          {release.name}
        </h2>
        <a
          href={release.url}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1 text-sm text-fg-muted hover:text-fg"
        >
          Release notes <ArrowUpRight className="size-3.5" aria-hidden="true" />
          <span className="sr-only"> (opens in a new tab)</span>
        </a>
      </div>
      <dl className="grid grid-cols-[repeat(auto-fit,minmax(12rem,1fr))] gap-px bg-hairline">
        {rows.map((r) => (
          <div key={r.label} className="bg-surface-1 px-5 py-4">
            <dt className="text-xs text-fg-muted">{r.label}</dt>
            <dd className="mt-1 truncate text-sm">{r.value}</dd>
          </div>
        ))}
      </dl>
      {primary.sha256 && (
        <div className="flex items-center gap-3 border-t border-hairline px-5 py-4">
          <Hash className="size-4 shrink-0 text-fg-muted" aria-hidden="true" />
          <div className="min-w-0 flex-1">
            <p className="text-xs text-fg-muted">SHA-256 of {primary.name}</p>
            <p className="truncate font-mono text-[13px]" title={primary.sha256}>
              {primary.sha256}
            </p>
          </div>
          <CopyButton text={primary.sha256} label="Copy SHA-256 checksum" />
        </div>
      )}
      {primary.sha256 && (
        <p className="border-t border-hairline px-5 py-3 text-xs text-fg-muted">
          Check it in PowerShell:{' '}
          <code className="font-mono text-fg">Get-FileHash .\{primary.name}</code>
        </p>
      )}
      {msix && (
        <a
          href={msix.url}
          className="flex items-center gap-2 border-t border-hairline px-5 py-4 text-sm text-fg-muted hover:text-fg"
        >
          <Package className="size-4" aria-hidden="true" />
          MSIX package: {msix.name} ({formatBytes(msix.size)})
        </a>
      )}
    </section>
  )
}
