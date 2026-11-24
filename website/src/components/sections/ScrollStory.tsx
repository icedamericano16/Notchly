import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { useEffect, useRef, useState } from 'react'
import { Reveal } from '@/components/common/Reveal'
import { SectionHeading } from '@/components/common/SectionHeading'
import { DynamicIsland } from '@/components/island/DynamicIsland'
import { storySteps, type StoryStep } from '@/content/home'
import { useLenis } from '@/hooks/useLenis'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/cn'
import { easeOutExpo } from '@/lib/motion'

/**
 * "How it lives": on desktop a GSAP ScrollTrigger pins the stage while the island morphs
 * on the left and the copy changes on the right. On mobile, or with reduced motion, it's
 * a plain stack of cards (no pinning, no GSAP download).
 */
export default function ScrollStory() {
  const desktop = useMediaQuery('(min-width: 1024px)')
  const reduced = useReducedMotion()

  return (
    <section aria-labelledby="story-title" className="relative pt-28 sm:pt-36">
      <div className="container-page">
        <SectionHeading
          id="story-title"
          eyebrow="How it lives"
          title="One notch. Everything that's happening."
          description="The most important thing owns the pill: a call beats a timer, a timer beats a download, a download beats your music. Scroll through a day with Notchly."
        />
      </div>
      {desktop && !reduced ? <PinnedStory /> : <StackedStory />}
    </section>
  )
}

function Stage({ step, size }: { step: StoryStep; size: 'md' | 'sm' }) {
  return (
    <div
      className={cn(
        'relative aspect-[16/11] w-full overflow-hidden rounded-card border border-hairline bg-wallpaper shadow-island',
        // Tall enough for the open control center.
        size === 'md' ? 'min-h-[27rem]' : 'min-h-[25rem]',
      )}
    >
      {/* Menu-bar hairline: the top edge of a screen. */}
      <div className="absolute inset-x-0 top-0 h-px bg-white/15" />
      <div className="absolute top-3 left-1/2 -translate-x-1/2">
        <DynamicIsland
          activity={step.activity}
          expanded={Boolean(step.expanded)}
          interactive={false}
          size={size}
        />
      </div>
      {step.expanded && (
        <m.span
          aria-hidden="true"
          className="absolute top-[52%] left-[62%] block size-5"
          initial={{ opacity: 0, x: 40, y: 30 }}
          animate={{ opacity: 1, x: 0, y: 0 }}
          transition={{ duration: 0.6, ease: easeOutExpo }}
        >
          {/* Pointer, to say "hover". */}
          <svg viewBox="0 0 24 24" className="size-5 drop-shadow">
            <path
              d="M4 2l16 9-7 2-3 7z"
              className="fill-fg stroke-canvas"
              strokeWidth="1.5"
              strokeLinejoin="round"
            />
          </svg>
        </m.span>
      )}
    </div>
  )
}

function PinnedStory() {
  const pinRef = useRef<HTMLDivElement>(null)
  const [active, setActive] = useState(0)
  const lenis = useLenis()
  const n = storySteps.length

  useEffect(() => {
    let cancelled = false
    let revert: (() => void) | undefined
    void Promise.all([import('gsap'), import('gsap/ScrollTrigger')]).then(
      ([{ gsap }, { ScrollTrigger }]) => {
        if (cancelled || !pinRef.current) return
        gsap.registerPlugin(ScrollTrigger)
        const ctx = gsap.context(() => {
          ScrollTrigger.create({
            trigger: pinRef.current,
            start: 'top top',
            end: `+=${(n - 1) * 70}%`,
            pin: true,
            snap: { snapTo: 1 / (n - 1), duration: { min: 0.2, max: 0.5 }, delay: 0.05 },
            onUpdate: (self) => setActive(Math.round(self.progress * (n - 1))),
          })
        })
        revert = () => ctx.revert()
      },
    )
    return () => {
      cancelled = true
      revert?.()
    }
  }, [n])

  // Keep ScrollTrigger in step with Lenis' smoothed scroll.
  useEffect(() => {
    if (!lenis) return
    let off: (() => void) | undefined
    void import('gsap/ScrollTrigger').then(({ ScrollTrigger }) => {
      const update = () => ScrollTrigger.update()
      off = lenis.on('scroll', update)
    })
    return () => off?.()
  }, [lenis])

  const step = storySteps[active] ?? storySteps[0]
  if (!step) return null

  return (
    // The outer div belongs to React; GSAP wraps only the inner (pinned) one.
    <div className="mt-8">
      <div ref={pinRef} className="flex h-dvh items-center">
        <div className="container-page grid grid-cols-[1.15fr_1fr] items-center gap-16">
          <Stage step={step} size="md" />

          <ol className="flex flex-col gap-2" aria-label="Notchly through the day">
            {storySteps.map((s, i) => {
              const isActive = i === active
              return (
                <li
                  key={s.id}
                  aria-current={isActive ? 'step' : undefined}
                  className="relative border-l border-hairline py-3 pl-6"
                >
                  <span
                    aria-hidden="true"
                    className={cn(
                      'absolute top-0 -left-px h-full w-px origin-top bg-island-accent transition-transform duration-500 ease-out-expo',
                      isActive ? 'scale-y-100' : 'scale-y-0',
                    )}
                  />
                  {/* Small text stays at fg-muted (4.5:1+); only the large titles dim. */}
                  <p className="font-mono text-[11px] tracking-widest text-fg-muted uppercase">
                    {s.eyebrow}
                  </p>
                  <h3
                    className={cn(
                      'mt-1 text-2xl font-semibold tracking-tightish transition-colors duration-300',
                      isActive ? 'text-fg' : 'text-fg-faint',
                    )}
                  >
                    {s.title}
                  </h3>
                  <AnimatePresence initial={false} mode="wait">
                    {isActive ? (
                      <m.p
                        key="body"
                        className="mt-3 max-w-md leading-relaxed text-fg-muted"
                        initial={{ opacity: 0, y: 8, filter: 'blur(4px)' }}
                        animate={{ opacity: 1, y: 0, filter: 'blur(0px)' }}
                        exit={{ opacity: 0, transition: { duration: 0.12 } }}
                        transition={{ duration: 0.45, ease: easeOutExpo }}
                      >
                        {s.body}
                      </m.p>
                    ) : (
                      <p key="sr" className="sr-only">
                        {s.body}
                      </p>
                    )}
                  </AnimatePresence>
                </li>
              )
            })}
          </ol>
        </div>
      </div>
    </div>
  )
}

function StackedStory() {
  const small = useMediaQuery('(max-width: 639px)')
  return (
    <ol className="container-page mt-12 flex flex-col gap-6" aria-label="Notchly through the day">
      {storySteps.map((s) => (
        <li key={s.id}>
          <Reveal className="grid gap-6 rounded-card border border-hairline bg-surface-1 p-3 sm:p-4 md:grid-cols-2 md:items-center md:gap-10">
            <Stage step={s} size={small ? 'sm' : 'md'} />
            <div className="px-2 pb-3 md:px-0">
              <p className="font-mono text-[11px] tracking-widest text-fg-muted uppercase">
                {s.eyebrow}
              </p>
              <h3 className="mt-2 text-2xl font-semibold tracking-tightish">{s.title}</h3>
              <p className="mt-3 leading-relaxed text-fg-muted">{s.body}</p>
            </div>
          </Reveal>
        </li>
      ))}
    </ol>
  )
}
