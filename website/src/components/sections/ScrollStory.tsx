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
 * "How it lives". On desktop the steps scroll normally while the island stage stays in
 * view beside them (CSS sticky, so scrolling never stops); a GSAP ScrollTrigger per step
 * switches the island as that step reaches the middle of the screen. On mobile, or with
 * reduced motion, it's a plain stack of cards (and GSAP is never downloaded).
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
      {desktop && !reduced ? <StickyStory /> : <StackedStory />}
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

function StickyStory() {
  const listRef = useRef<HTMLOListElement>(null)
  const [active, setActive] = useState(0)
  const lenis = useLenis()

  // One trigger per step: it becomes active while it crosses the middle of the viewport.
  useEffect(() => {
    let cancelled = false
    let revert: (() => void) | undefined
    void Promise.all([import('gsap'), import('gsap/ScrollTrigger')]).then(
      ([{ gsap }, { ScrollTrigger }]) => {
        const list = listRef.current
        if (cancelled || !list) return
        gsap.registerPlugin(ScrollTrigger)
        const ctx = gsap.context(() => {
          Array.from(list.children).forEach((item, i) => {
            ScrollTrigger.create({
              trigger: item,
              start: 'top 55%',
              end: 'bottom 55%',
              onToggle: (self) => {
                if (self.isActive) setActive(i)
              },
            })
          })
        })
        revert = () => ctx.revert()
      },
    )
    return () => {
      cancelled = true
      revert?.()
    }
  }, [])

  // Keep ScrollTrigger in step with Lenis' smoothed scroll.
  useEffect(() => {
    if (!lenis) return
    let off: (() => void) | undefined
    void import('gsap/ScrollTrigger').then(({ ScrollTrigger }) => {
      off = lenis.on('scroll', () => ScrollTrigger.update())
    })
    return () => off?.()
  }, [lenis])

  const step = storySteps[active] ?? storySteps[0]
  if (!step) return null

  return (
    <div className="container-page mt-8 grid grid-cols-[1.15fr_1fr] gap-16">
      {/* The stage stays in view while the steps scroll past it. */}
      <div>
        <div className="sticky top-[max(6rem,calc(50dvh-14rem))]">
          <Stage step={step} size="md" />
        </div>
      </div>

      <ol ref={listRef} aria-label="Notchly through the day" className="pb-[20dvh]">
        {storySteps.map((s, i) => {
          const isActive = i === active
          return (
            <li
              key={s.id}
              aria-current={isActive ? 'step' : undefined}
              className="flex min-h-[70dvh] flex-col justify-center py-10"
            >
              <p className="font-mono text-[11px] tracking-widest text-fg-muted uppercase">
                {s.eyebrow}
              </p>
              {/* Only the large title dims (large text keeps 3:1+); body text stays 4.5:1+. */}
              <h3
                className={cn(
                  'mt-2 text-3xl font-semibold tracking-tightish transition-colors duration-500',
                  isActive ? 'text-fg' : 'text-fg-faint',
                )}
              >
                {s.title}
              </h3>
              <p className="mt-4 max-w-md text-[17px] leading-relaxed text-fg-muted">{s.body}</p>
            </li>
          )
        })}
      </ol>
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
