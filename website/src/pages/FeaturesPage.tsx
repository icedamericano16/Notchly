import { DownloadButton } from '@/components/common/DownloadButton'
import { PageHeader } from '@/components/common/PageHeader'
import { RevealGroup, RevealItem } from '@/components/common/Reveal'
import { Seo } from '@/components/common/Seo'
import { FeatureIcon } from '@/components/sections/FeatureIcon'
import { SpotlightCard } from '@/components/reactbits/SpotlightCard'
import { featureGroups } from '@/content/features'
import { site } from '@/content/site'

export default function FeaturesPage() {
  const total = featureGroups.reduce((n, g) => n + g.items.length, 0)

  return (
    <>
      <Seo page="features" />
      <PageHeader
        eyebrow="Features"
        title="Everything in the notch."
        description={`${featureGroups.length} groups, ${total} headline features, ${site.facts.toolCount} tools in all. Every one is free and included.`}
      />

      {/* Jump links: sticky under the navbar. */}
      <nav
        aria-label="Feature groups"
        className="sticky top-[4.75rem] z-20 border-y border-hairline bg-canvas/80 backdrop-blur-glass"
      >
        <ul className="container-page flex [scrollbar-width:none] gap-1 overflow-x-auto py-2">
          {featureGroups.map((g) => (
            <li key={g.id} className="shrink-0">
              <a
                href={`#${g.id}`}
                className="block rounded-full px-3 py-1.5 text-sm whitespace-nowrap text-fg-muted transition-colors hover:bg-white/[0.06] hover:text-fg"
              >
                {g.title}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <div className="container-page flex flex-col gap-24 pt-16">
        {featureGroups.map((g) => (
          <section key={g.id} id={g.id} aria-labelledby={`${g.id}-title`} className="scroll-mt-36">
            <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
              <div className="flex items-center gap-4">
                <FeatureIcon id={g.icon} className="size-11" />
                <div>
                  <h2 id={`${g.id}-title`} className="text-3xl text-display sm:text-4xl">
                    {g.title}
                  </h2>
                  <p className="mt-1 text-fg-muted">{g.summary}</p>
                </div>
              </div>
              <p className="font-mono text-xs text-fg-muted">{g.items.length} features</p>
            </div>

            <RevealGroup
              as="ul"
              stagger={0.05}
              className="mt-8 grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
            >
              {g.items.map((item) => (
                <RevealItem as="li" key={item.name} className="flex">
                  <SpotlightCard className="flex w-full flex-col gap-2 p-5">
                    <h3 className="font-medium">{item.name}</h3>
                    <p className="text-sm leading-relaxed text-fg-muted">{item.description}</p>
                  </SpotlightCard>
                </RevealItem>
              ))}
            </RevealGroup>
          </section>
        ))}
      </div>

      <section className="container-page mt-32 flex flex-col items-center gap-6 text-center">
        <h2 className="text-4xl text-display sm:text-5xl">All of it, free.</h2>
        <DownloadButton showMeta />
      </section>
    </>
  )
}
