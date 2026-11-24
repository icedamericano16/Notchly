import { ArrowRight } from 'lucide-react'
import type { ComponentType } from 'react'
import { Link } from 'react-router'
import { RevealGroup, RevealItem } from '@/components/common/Reveal'
import { SectionHeading } from '@/components/common/SectionHeading'
import { SpotlightCard } from '@/components/reactbits/SpotlightCard'
import { featureGroups } from '@/content/features'
import { useOffscreenPause } from '@/hooks/useOffscreenPause'
import { cn } from '@/lib/cn'
import {
  AskVisual,
  CodeVisual,
  LiveVisual,
  ShelfVisual,
  SystemVisual,
  ThemeVisual,
  TodoVisual,
} from './BentoVisuals'
import { FeatureIcon } from './FeatureIcon'

/** Visual and grid placement per group id. */
const layout: Record<string, { Visual: ComponentType; span: string }> = {
  'live-activities': { Visual: LiveVisual, span: 'lg:col-span-4' },
  'smart-tools': { Visual: AskVisual, span: 'lg:col-span-2' },
  productivity: { Visual: TodoVisual, span: 'lg:col-span-2' },
  files: { Visual: ShelfVisual, span: 'lg:col-span-2' },
  system: { Visual: SystemVisual, span: 'lg:col-span-2' },
  look: { Visual: ThemeVisual, span: 'lg:col-span-2' },
  developers: { Visual: CodeVisual, span: 'lg:col-span-4' },
}

export function Bento() {
  const ref = useOffscreenPause<HTMLElement>()
  return (
    <section
      ref={ref}
      aria-labelledby="features-title"
      className="motion-deferred container-page pt-24 sm:pt-28"
    >
      <SectionHeading
        id="features-title"
        eyebrow="Features"
        title="Forty-plus tools, one notch."
        description="Live activities, an AI that can run on your own PC, your day at a glance, and a pocket for files. All free, all local-first."
      />

      <RevealGroup
        as="ul"
        stagger={0.07}
        className="mt-14 grid grid-flow-dense grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-6"
      >
        {featureGroups.map((group) => {
          const { Visual, span } = layout[group.id] ?? { Visual: CodeVisual, span: 'lg:col-span-2' }
          const wide = group.id === 'developers'
          return (
            <RevealItem
              as="li"
              key={group.id}
              className={cn('flex', span, wide && 'sm:col-span-2')}
            >
              <SpotlightCard className="flex w-full flex-col gap-5 p-5 sm:p-6">
                <div className="min-w-0">
                  <Visual />
                </div>
                <div className="flex flex-col gap-3">
                  <div className="flex items-center gap-3">
                    <FeatureIcon id={group.icon} />
                    <h3 className="text-lg font-semibold tracking-tightish">{group.title}</h3>
                  </div>
                  <p className="text-[15px] leading-relaxed text-fg-muted">{group.summary}</p>
                  <ul className="flex flex-wrap gap-1.5" aria-label={`${group.title} features`}>
                    {group.items.map((item) => (
                      <li
                        key={item.name}
                        className="rounded-full border border-hairline bg-surface-2/60 px-2.5 py-1 text-xs text-fg-muted"
                      >
                        {item.name}
                      </li>
                    ))}
                  </ul>
                </div>
              </SpotlightCard>
            </RevealItem>
          )
        })}
      </RevealGroup>

      <div className="mt-10 flex justify-center">
        <Link
          to="/features"
          className="group inline-flex items-center gap-2 rounded-full border border-hairline px-5 py-2.5 text-sm text-fg-muted transition-colors hover:border-hairline-strong hover:text-fg"
        >
          See every feature
          <ArrowRight
            className="size-4 transition-transform group-hover:translate-x-0.5"
            aria-hidden="true"
          />
        </Link>
      </div>
    </section>
  )
}
