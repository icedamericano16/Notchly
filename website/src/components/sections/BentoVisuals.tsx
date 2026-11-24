/* Small looping visuals for the bento cards. Decorative (aria-hidden), transform/opacity
   only, and paused offscreen by the parent section. */
import { Check, FileText, Image, Search, Sparkles } from 'lucide-react'
import { useInView } from 'motion/react'
import { useRef } from 'react'
import { DynamicIsland } from '@/components/island/DynamicIsland'
import { useIslandCycle } from '@/components/island/useIslandCycle'
import { themePresets } from '@/content/customise'
import { heroSequence } from '@/content/island'
import { useMediaQuery } from '@/hooks/useMediaQuery'

export function LiveVisual() {
  const ref = useRef<HTMLDivElement>(null)
  const inView = useInView(ref)
  const small = useMediaQuery('(max-width: 639px)')
  const { activity } = useIslandCycle(heroSequence, { intervalMs: 2600, paused: !inView })
  return (
    <div
      ref={ref}
      aria-hidden="true"
      className="relative flex h-44 justify-center overflow-hidden rounded-xl bg-desktop pt-4 sm:h-56"
    >
      <DynamicIsland activity={activity} interactive={false} size={small ? 'sm' : 'md'} />
    </div>
  )
}

export function AskVisual() {
  return (
    <div
      aria-hidden="true"
      className="flex h-36 flex-col justify-center gap-2 rounded-xl bg-surface-2 p-4"
    >
      <div className="flex items-center gap-2 rounded-full bg-canvas px-3 py-2 text-[13px] text-fg-muted">
        <Search className="size-3.5" />
        <span>translate “good morning” to Nepali</span>
        <span className="ml-auto h-3.5 w-px animate-[fade-in_1s_steps(2)_infinite] bg-fg" />
      </div>
      <div
        className="flex animate-float items-start gap-2 self-start rounded-2xl rounded-tl-sm bg-white/[0.06] px-3 py-2 text-[13px]"
        style={{ animationDuration: '3s' }}
      >
        <Sparkles className="mt-0.5 size-3.5 text-brand-indigo" />
        शुभ प्रभात
      </div>
    </div>
  )
}

const todos = [
  { text: 'Call mom', when: '6 PM', done: true },
  { text: 'Standup', when: 'Tomorrow 9:00', done: false },
  { text: 'Water plants', when: 'Every day 8 AM', done: false },
]

export function TodoVisual() {
  return (
    <ul
      aria-hidden="true"
      className="flex h-36 flex-col justify-center gap-2 rounded-xl bg-surface-2 p-4"
    >
      {todos.map((t, i) => (
        <li
          key={t.text}
          className="flex animate-rise-in items-center gap-2.5 text-[13px]"
          style={{ animationDelay: `${i * 0.15}s` }}
        >
          <span
            className={
              t.done
                ? 'flex size-4 items-center justify-center rounded-full bg-live-green text-canvas'
                : 'size-4 rounded-full border border-hairline-strong'
            }
          >
            {t.done && <Check className="size-3" />}
          </span>
          <span className={t.done ? 'text-fg-muted line-through' : ''}>{t.text}</span>
          <span className="ml-auto font-mono text-[11px] text-fg-muted">{t.when}</span>
        </li>
      ))}
    </ul>
  )
}

const files = [
  { name: 'report.pdf', Icon: FileText, delay: '0s' },
  { name: 'photo.png', Icon: Image, delay: '1.2s' },
  { name: 'notes.txt', Icon: FileText, delay: '2.1s' },
]

export function ShelfVisual() {
  return (
    <div
      aria-hidden="true"
      className="flex h-36 items-center justify-center gap-3 rounded-xl bg-surface-2 p-4"
    >
      {files.map(({ name, Icon, delay }) => (
        <div
          key={name}
          className="flex animate-float flex-col items-center gap-1.5 rounded-xl border border-hairline bg-canvas px-3 py-2.5"
          style={{ animationDelay: delay, animationDuration: '2.8s' }}
        >
          <Icon className="size-5 text-fg-muted" />
          <span className="text-[11px] text-fg-muted">{name}</span>
        </div>
      ))}
    </div>
  )
}

const meters = [
  { label: 'CPU', delay: '0s' },
  { label: 'RAM', delay: '0.35s' },
  { label: 'GPU', delay: '0.7s' },
]

export function SystemVisual() {
  return (
    <div
      aria-hidden="true"
      className="flex h-36 items-end justify-center gap-6 rounded-xl bg-surface-2 p-4"
    >
      {meters.map((meter) => (
        <div key={meter.label} className="flex flex-col items-center gap-2">
          <div className="flex h-20 items-end gap-1">
            {[0, 1, 2, 3].map((i) => (
              <span
                key={i}
                className="h-full w-2 origin-bottom animate-eq rounded-sm bg-island-accent"
                style={{
                  animationDelay: `calc(${meter.delay} + ${i * 0.2}s)`,
                  animationDuration: `${1.6 + i * 0.3}s`,
                }}
              />
            ))}
          </div>
          <span className="font-mono text-[11px] text-fg-muted">{meter.label}</span>
        </div>
      ))}
      <div className="flex flex-col gap-1.5 self-center">
        <span className="size-2.5 rounded-full bg-live-orange" />
        <span className="size-2.5 rounded-full bg-live-green" />
      </div>
    </div>
  )
}

export function ThemeVisual() {
  return (
    <div
      aria-hidden="true"
      className="flex h-36 flex-wrap content-center items-center justify-center gap-2.5 rounded-xl bg-surface-2 p-4"
    >
      {themePresets.map((t, i) => (
        <span
          key={t.id}
          className="size-8 animate-float rounded-full ring-1 ring-white/10"
          style={{
            backgroundImage: `linear-gradient(135deg, ${t.stops[0]}, ${t.stops[1]}, ${t.stops[2]})`,
            animationDelay: `${i * 0.25}s`,
            animationDuration: '3s',
          }}
        />
      ))}
    </div>
  )
}

export function CodeVisual() {
  return (
    <pre
      aria-hidden="true"
      className="overflow-hidden rounded-xl bg-surface-2 p-4 font-mono text-[12px] leading-relaxed text-fg-muted"
    >
      <span className="text-brand-indigo">notchly</span> live start --id build-42 --title Build
      {'\n'}
      <span className="text-brand-indigo">notchly</span> live update --id build-42 --progress 0.8
      {'\n'}
      <span className="text-brand-indigo">notchly</span> live end --id build-42 --final{' '}
      <span className="text-live-green">"Done in 2m 14s"</span>
    </pre>
  )
}
