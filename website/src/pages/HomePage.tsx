import { ArrowRight } from 'lucide-react'
import { Link } from 'react-router'
import { GitHubIcon, WindowsIcon } from '@/components/common/BrandIcons'
import { Seo } from '@/components/common/Seo'
import { Button } from '@/components/ui/button'
import { hero } from '@/content/home'
import { site } from '@/content/site'

/**
 * Step 1 preview: the hero's type, colour and CTAs on the real tokens, so the scaffold can
 * be judged. The island (step 2), backgrounds, video and the rest of the sections replace
 * this in steps 2–5.
 */
export default function HomePage() {
  const [lead] = hero.headline.split(hero.headlineAccent)

  return (
    <>
      <Seo page="home" />
      <section className="relative isolate overflow-hidden pt-36 pb-24 sm:pt-44">
        {/* Soft accent glow behind the island position. Static; the animated background
            arrives with the hero in step 3. */}
        <div
          aria-hidden="true"
          className="absolute top-0 left-1/2 -z-10 h-72 w-[42rem] max-w-[90vw] -translate-x-1/2 rounded-full bg-brand-gradient opacity-20 blur-3xl"
        />

        <div className="container-page flex flex-col items-center text-center">
          {/* Placeholder for the interactive island (step 2). */}
          <div
            role="img"
            aria-label="Dynamic Island demo placeholder"
            className="mb-14 flex h-9 w-32 items-center justify-center rounded-full bg-island shadow-island"
          >
            <span className="font-mono text-[10px] tracking-wider text-fg-faint uppercase">
              island · step 2
            </span>
          </div>

          <p className="inline-flex items-center gap-2 rounded-full border border-hairline bg-surface-1 px-3 py-1 text-xs text-fg-muted">
            <span className="size-1.5 rounded-full bg-live-green" aria-hidden="true" />
            {hero.eyebrow}
          </p>

          <h1 className="mt-6 max-w-4xl text-5xl text-display sm:text-7xl lg:text-[5.5rem]">
            {lead}
            <span className="text-brand-gradient">{hero.headlineAccent}</span>
          </h1>

          <p className="mt-6 max-w-2xl text-lg text-pretty text-fg-muted sm:text-xl">
            {hero.subheadline}
          </p>

          <div className="mt-10 flex flex-col items-center gap-3 sm:flex-row">
            <Button asChild size="lg" className="h-12 rounded-full px-6 text-[15px]">
              <Link to="/download">
                <WindowsIcon className="size-4" />
                {hero.primaryCta}
              </Link>
            </Button>
            <Button
              asChild
              size="lg"
              variant="ghost"
              className="group h-12 rounded-full px-6 text-[15px] text-fg-muted hover:text-fg"
            >
              <a href={site.repo.url} target="_blank" rel="noopener noreferrer">
                <GitHubIcon className="size-4" />
                {hero.secondaryCta}
                <ArrowRight className="transition-transform duration-200 group-hover:translate-x-0.5" />
                <span className="sr-only"> (opens in a new tab)</span>
              </a>
            </Button>
          </div>

          <p className="mt-6 font-mono text-xs text-fg-muted">{hero.microcopy}</p>
        </div>
      </section>
    </>
  )
}
