import { Reveal, RevealGroup, RevealItem } from '@/components/common/Reveal'
import { CountUp } from '@/components/reactbits/CountUp'
import { stats, whyRust } from '@/content/home'

export function Stats() {
  return (
    <section aria-labelledby="stats-title" className="container-page pt-32 sm:pt-40">
      <div className="overflow-hidden rounded-frame border border-hairline bg-surface-1/50">
        <RevealGroup
          as="ul"
          className="grid grid-cols-2 gap-px border-b border-hairline bg-hairline lg:grid-cols-4"
        >
          {stats.map((s) => (
            <RevealItem
              as="li"
              key={s.label}
              className="flex flex-col gap-2 bg-surface-1 p-6 sm:p-8"
            >
              <p className="text-4xl font-semibold tracking-display tabular-nums sm:text-6xl">
                <CountUp to={s.value} decimals={s.decimals ?? 0} />
                <span className="text-fg-muted">{s.suffix}</span>
              </p>
              <p className="text-sm text-fg-muted">{s.label}</p>
            </RevealItem>
          ))}
        </RevealGroup>
        <Reveal className="grid gap-6 p-6 sm:p-10 lg:grid-cols-[1fr_2fr]">
          <h2 id="stats-title" className="text-3xl text-display sm:text-4xl">
            {whyRust.title}
          </h2>
          <div className="flex flex-col gap-4 leading-relaxed text-fg-muted">
            {whyRust.paragraphs.map((p) => (
              <p key={p}>{p}</p>
            ))}
          </div>
        </Reveal>
      </div>
    </section>
  )
}
