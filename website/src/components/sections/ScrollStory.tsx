import * as m from 'motion/react-m'
import { useEffect, useRef, useState, type RefObject } from 'react'
import { SectionHeading } from '@/components/common/SectionHeading'
import { DynamicIsland } from '@/components/island/DynamicIsland'
import { storySteps, type StoryStep } from '@/content/home'
import { useMediaQuery } from '@/hooks/useMediaQuery'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/cn'
import { easeOutExpo } from '@/lib/motion'

/**
 * "How it lives": five moments, scrolled inside the section. On desktop the steps scroll
 * (and snap) in a panel beside the island stage, which shows the step in the middle; the
 * page carries on once the panel reaches its end. On phones and tablets the steps are a
 * swipeable row of cards. No scrollbars, no pinning, no GSAP.
 */
export default function ScrollStory() {
  const desktop = useMediaQuery('(min-width: 1024px)')

  return (
    <section aria-labelledby="story-title" className="relative pt-28 sm:pt-36">
      <div className="container-page">
        <SectionHeading
          id="story-title"
          eyebrow="How it lives"
          title="One notch. Everything that's happening."
          description="The most important thing owns the pill: a call beats a timer, a timer beats a download, a download beats your music."
        />
      </div>
      {desktop ? <PanelStory /> : <CarouselStory />}
    </section>
  )
}

function Stage({ step, size }: { step: StoryStep; size: 'md' | 'sm' }) {
  return (
    <div
      className={cn(
        'relative w-full overflow-hidden rounded-card border border-hairline bg-desktop shadow-island',
        // Desktop: one stage, tall enough for the open control center. Cards: all the same
        // height; the open control center is shown scaled down to fit.
        size === 'md' ? 'h-[28rem]' : 'h-52',
      )}
    >
      {/* The screen's top edge. */}
      <div className="absolute inset-x-0 top-0 h-px bg-white/15" />
      <div
        className={cn(
          'absolute top-3 left-1/2 -translate-x-1/2',
          size === 'sm' && step.expanded && 'top-2 origin-top scale-[0.5]',
        )}
      >
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
          className="absolute top-[56%] left-[64%] block size-5"
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

/** Which item is in the middle of the scroller (vertical) or mostly visible (horizontal). */
function useActiveItem(scroller: RefObject<HTMLElement | null>, axis: 'y' | 'x'): number {
  const [active, setActive] = useState(0)
  useEffect(() => {
    const root = scroller.current
    if (!root) return
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) setActive(Number((e.target as HTMLElement).dataset.index))
        }
      },
      axis === 'y' ? { root, rootMargin: '-48% 0px -48% 0px' } : { root, threshold: 0.6 },
    )
    root.querySelectorAll('[data-index]').forEach((el) => io.observe(el))
    return () => io.disconnect()
  }, [scroller, axis])
  return active
}

function scrollItemIntoView(
  scroller: HTMLElement | null,
  index: number,
  axis: 'y' | 'x',
  smooth: boolean,
) {
  const item = scroller?.querySelector<HTMLElement>(`[data-index="${index}"]`)
  if (!scroller || !item) return
  const behavior = smooth ? 'smooth' : 'auto'
  if (axis === 'y') {
    scroller.scrollTo({
      top: item.offsetTop - (scroller.clientHeight - item.offsetHeight) / 2,
      behavior,
    })
  } else {
    scroller.scrollTo({
      left: item.offsetLeft - (scroller.clientWidth - item.offsetWidth) / 2,
      behavior,
    })
  }
}

interface StepDotsProps {
  active: number
  onSelect: (i: number) => void
  vertical?: boolean
  className?: string
}

function StepDots({ active, onSelect, vertical = false, className }: StepDotsProps) {
  return (
    <div
      role="group"
      aria-label="Steps"
      className={cn('flex gap-2', vertical ? 'flex-col' : 'flex-row justify-center', className)}
    >
      {storySteps.map((s, i) => (
        <button
          key={s.id}
          type="button"
          aria-label={`Step ${i + 1}: ${s.title}`}
          aria-current={i === active ? 'step' : undefined}
          onClick={() => onSelect(i)}
          className="flex size-5 items-center justify-center"
        >
          <span
            className={cn(
              'block size-1.5 rounded-full transition-colors duration-300',
              i === active ? 'bg-fg' : 'bg-white/25',
            )}
          />
        </button>
      ))}
    </div>
  )
}

function StepText({ step, active, large }: { step: StoryStep; active: boolean; large?: boolean }) {
  return (
    <>
      <p className="font-mono text-[11px] tracking-widest text-fg-muted uppercase">
        {step.eyebrow}
      </p>
      {/* Only the large title dims (large text keeps 3:1+); body text stays 4.5:1+. */}
      <h3
        className={cn(
          'mt-2 font-semibold tracking-tightish transition-colors duration-500',
          large ? 'text-[1.7rem] leading-tight' : 'text-xl',
          active ? 'text-fg' : 'text-fg-faint',
        )}
      >
        {step.title}
      </h3>
      <p className="mt-3 max-w-md leading-relaxed text-fg-muted">{step.body}</p>
    </>
  )
}

/** Desktop: stage on the left, a scroll panel of steps on the right. */
function PanelStory() {
  const scroller = useRef<HTMLDivElement>(null)
  const active = useActiveItem(scroller, 'y')
  const reduced = useReducedMotion()
  const step = storySteps[active] ?? storySteps[0]
  if (!step) return null

  return (
    <div className="container-page mt-12 grid grid-cols-[1.15fr_1fr] items-center gap-12">
      <Stage step={step} size="md" />

      <div className="relative h-[28rem]">
        {/* data-lenis-prevent: wheel events scroll this panel natively, not the page. */}
        <div
          ref={scroller}
          data-lenis-prevent
          tabIndex={0}
          role="region"
          aria-label="Notchly through the day: scroll for all five moments"
          className="absolute inset-0 snap-y snap-mandatory [scrollbar-width:none] overflow-y-auto rounded-card pr-10 focus-visible:outline-offset-4 [&::-webkit-scrollbar]:hidden"
        >
          {/* Spacers let the first and last step reach the middle. */}
          <div aria-hidden="true" className="h-[calc(50%-5.5rem)]" />
          <ol>
            {storySteps.map((s, i) => (
              <li key={s.id} data-index={i} className="snap-center py-5">
                <StepText step={s} active={i === active} large />
              </li>
            ))}
          </ol>
          <div aria-hidden="true" className="h-[calc(50%-5.5rem)]" />
        </div>

        {/* Soft edges instead of a hard cut (and instead of a scrollbar). */}
        <div className="pointer-events-none absolute inset-x-0 top-0 h-20 bg-gradient-to-b from-canvas to-transparent" />
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-20 bg-gradient-to-t from-canvas to-transparent" />

        <StepDots
          vertical
          active={active}
          onSelect={(i) => scrollItemIntoView(scroller.current, i, 'y', !reduced)}
          className="absolute top-1/2 right-0 -translate-y-1/2"
        />
      </div>
    </div>
  )
}

/** Phones and tablets: swipe through the steps; each card has its own stage. */
function CarouselStory() {
  const scroller = useRef<HTMLDivElement>(null)
  const active = useActiveItem(scroller, 'x')
  const reduced = useReducedMotion()

  return (
    <div className="mt-10">
      <div
        ref={scroller}
        tabIndex={0}
        role="region"
        aria-label="Notchly through the day: swipe for all five moments"
        className="flex snap-x snap-mandatory scroll-px-4 [scrollbar-width:none] items-start gap-4 overflow-x-auto px-4 pb-2 sm:scroll-px-6 sm:px-6 [&::-webkit-scrollbar]:hidden"
      >
        {storySteps.map((s, i) => (
          <article
            key={s.id}
            data-index={i}
            aria-roledescription="slide"
            aria-label={`${i + 1} of ${storySteps.length}: ${s.title}`}
            className="w-[86%] shrink-0 snap-center rounded-card border border-hairline bg-surface-1 p-3 sm:w-[62%] md:w-[48%]"
          >
            <Stage step={s} size="sm" />
            <div className="px-2 pt-5 pb-3">
              <StepText step={s} active />
            </div>
          </article>
        ))}
      </div>
      <StepDots
        active={active}
        onSelect={(i) => scrollItemIntoView(scroller.current, i, 'x', !reduced)}
        className="mt-4"
      />
    </div>
  )
}
