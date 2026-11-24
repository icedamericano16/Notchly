import { useInView } from 'motion/react'
import { useEffect, useRef, useState } from 'react'
import { CopyButton } from '@/components/common/CopyButton'
import { Reveal } from '@/components/common/Reveal'
import { SectionHeading } from '@/components/common/SectionHeading'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { developer, snippets, type Snippet } from '@/content/developers'
import { site } from '@/content/site'
import { highlight } from '@/lib/highlight'

export function Developer() {
  return (
    <section aria-labelledby="dev-title" className="container-page pt-32 sm:pt-40">
      <div className="grid gap-12 lg:grid-cols-[1fr_1.3fr] lg:items-center">
        <div className="flex flex-col gap-6">
          <SectionHeading
            id="dev-title"
            align="start"
            eyebrow={developer.eyebrow}
            title={developer.title}
            description={developer.body}
          />
          <Reveal>
            <ul className="flex flex-wrap gap-2">
              {developer.limits.map((l) => (
                <li
                  key={l}
                  className="rounded-full border border-hairline px-3 py-1 font-mono text-xs text-fg-muted"
                >
                  {l}
                </li>
              ))}
            </ul>
          </Reveal>
          <Reveal>
            <a
              href={site.repo.sdkUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-fg-muted underline underline-offset-4 hover:text-fg"
            >
              SDKs and examples on GitHub<span className="sr-only"> (opens in a new tab)</span>
            </a>
          </Reveal>
        </div>

        <Reveal>
          <Tabs
            defaultValue={snippets[0]?.id}
            className="gap-0 overflow-hidden rounded-card border border-hairline bg-surface-1"
          >
            <div className="flex items-center justify-between border-b border-hairline px-2">
              <TabsList variant="line" aria-label="Code examples" className="h-12 gap-0">
                {snippets.map((s) => (
                  <TabsTrigger key={s.id} value={s.id} className="h-12 px-3 text-[13px]">
                    {s.label}
                  </TabsTrigger>
                ))}
              </TabsList>
            </div>
            {snippets.map((s) => (
              <TabsContent key={s.id} value={s.id} className="relative">
                <CodeBlock snippet={s} />
              </TabsContent>
            ))}
          </Tabs>
        </Reveal>
      </div>
    </section>
  )
}

/** Plain code first (readable at once), swapped for Shiki's colours once it loads. */
function CodeBlock({ snippet }: { snippet: Snippet }) {
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref, { once: true, margin: '200px' })
  const [html, setHtml] = useState<string | null>(null)

  useEffect(() => {
    if (!inView) return
    let alive = true
    void highlight(snippet.code, snippet.lang)
      .then((h) => alive && setHtml(h))
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [inView, snippet])

  return (
    <div ref={ref} className="relative">
      <p className="px-5 pt-4 font-mono text-[11px] text-fg-muted">{snippet.file}</p>
      <CopyButton
        text={snippet.code}
        label={`Copy ${snippet.label} example`}
        className="absolute top-3 right-3"
      />
      {html ? (
        <div
          className="overflow-x-auto px-5 py-4 font-mono text-[13px] leading-relaxed [&_pre]:!bg-transparent"
          // Site-authored code only; never user input.
          dangerouslySetInnerHTML={{ __html: html }}
        />
      ) : (
        <pre className="overflow-x-auto px-5 py-4 font-mono text-[13px] leading-relaxed text-fg/85">
          <code>{snippet.code}</code>
        </pre>
      )}
    </div>
  )
}
