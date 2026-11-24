import { Marquee } from '@/components/magicui/Marquee'
import { trustFacts } from '@/content/home'

export function TrustStrip() {
  return (
    <section aria-label="At a glance" className="border-y border-hairline bg-surface-1/40 py-5">
      <Marquee duration={36} label="At a glance">
        {trustFacts.map((fact) => (
          <span
            key={fact}
            className="flex items-center gap-8 px-4 font-mono text-sm whitespace-nowrap text-fg-muted"
          >
            {fact}
            <span aria-hidden="true" className="size-1 rounded-full bg-brand-indigo/70" />
          </span>
        ))}
      </Marquee>
    </section>
  )
}
