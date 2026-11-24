import { ArrowRight } from 'lucide-react'
import { AuroraBackground } from '@/components/common/AuroraBackground'
import { GitHubIcon } from '@/components/common/BrandIcons'
import { DownloadButton } from '@/components/common/DownloadButton'
import { VideoPlayer } from '@/components/common/VideoPlayer'
import { IslandDemo } from '@/components/island/IslandDemo'
import { BorderBeam } from '@/components/magicui/BorderBeam'
import { SplitText } from '@/components/reactbits/SplitText'
import { Button } from '@/components/ui/button'
import { hero } from '@/content/home'
import { heroVideo } from '@/content/media'
import { site } from '@/content/site'

/**
 * Above the fold everything animates with CSS (no waiting on JavaScript), so the headline,
 * the LCP element, paints on the first frame.
 */
export function Hero() {
  return (
    <section
      aria-labelledby="hero-title"
      className="relative isolate overflow-hidden pt-24 sm:pt-28"
    >
      <AuroraBackground />

      <div className="container-page flex flex-col items-center text-center">
        <IslandDemo className="mb-14 animate-fade-in sm:mb-16" />

        <p
          className="inline-flex animate-rise-in items-center gap-2 rounded-full border border-hairline bg-surface-1 px-3 py-1 text-xs text-fg-muted"
          style={{ animationDelay: '0.05s' }}
        >
          <span className="size-1.5 rounded-full bg-live-green" aria-hidden="true" />
          {hero.eyebrow}
        </p>

        <SplitText
          as="h1"
          text={hero.headline}
          accent={hero.headlineAccent}
          delay={0.1}
          id="hero-title"
          className="mt-6 max-w-4xl text-[2.75rem] text-display sm:text-7xl lg:text-[5.5rem]"
        />

        {/* Static on purpose: it's the LCP element, so it must paint on the first frame. */}
        <p className="mt-6 max-w-2xl text-lg text-pretty text-fg-muted sm:text-xl">
          {hero.subheadline}
        </p>

        <div
          className="mt-10 flex animate-rise-in flex-col items-center gap-3 sm:flex-row sm:items-start"
          style={{ animationDelay: '0.55s' }}
        >
          <DownloadButton label={hero.primaryCta} showMeta />
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

        <p
          className="mt-5 animate-fade-in font-mono text-xs text-fg-muted"
          style={{ animationDelay: '0.7s' }}
        >
          {hero.microcopy}
        </p>
      </div>

      {/* Desktop frame with the demo video. */}
      <div className="container-page mt-16 sm:mt-20">
        <figure
          className="relative mx-auto max-w-5xl animate-rise-in rounded-frame border border-hairline bg-surface-1/80 p-1.5 shadow-glow sm:p-2"
          style={{ animationDelay: '0.8s' }}
        >
          <BorderBeam duration={10} />
          <div className="overflow-hidden rounded-[calc(var(--radius-frame)-0.4rem)] bg-canvas">
            <VideoPlayer media={heroVideo} />
          </div>
          <figcaption className="sr-only">{heroVideo.caption}</figcaption>
        </figure>
      </div>
    </section>
  )
}
