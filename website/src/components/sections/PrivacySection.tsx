import { ArrowRight, ShieldCheck } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router'
import { RevealGroup, RevealItem } from '@/components/common/Reveal'
import { SectionHeading } from '@/components/common/SectionHeading'
import { Switch } from '@/components/common/Switch'
import { networkFeatures, privacyIntro } from '@/content/privacy'

/** The complete list of network features, each with a working toggle. */
export function PrivacySection() {
  const [on, setOn] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(networkFeatures.map((f) => [f.id, true])),
  )
  const offCount = networkFeatures.filter((f) => !on[f.id]).length

  return (
    <section aria-labelledby="privacy-title" className="container-page pt-32 sm:pt-40">
      <div className="grid gap-12 lg:grid-cols-[1fr_1.15fr] lg:items-start">
        <div className="flex flex-col gap-6 lg:sticky lg:top-28">
          <SectionHeading
            id="privacy-title"
            align="start"
            eyebrow={privacyIntro.eyebrow}
            title={privacyIntro.title}
            description={privacyIntro.body}
          />
          <p className="text-sm leading-relaxed text-fg-muted">{privacyIntro.note}</p>
          <Link
            to="/privacy"
            className="group inline-flex w-fit items-center gap-2 text-sm text-fg underline-offset-4 hover:underline"
          >
            How Notchly handles your data
            <ArrowRight
              className="size-4 transition-transform group-hover:translate-x-0.5"
              aria-hidden="true"
            />
          </Link>
        </div>

        <div className="rounded-card border border-hairline bg-surface-1/60 p-3 sm:p-4">
          <div className="flex items-center justify-between gap-3 px-2 pt-1 pb-3">
            <p className="flex items-center gap-2 text-sm font-medium">
              <ShieldCheck className="size-4 text-live-green" aria-hidden="true" />
              Settings › Privacy
            </p>
            <p className="font-mono text-xs text-fg-muted" aria-live="polite">
              {offCount === 0 ? 'All on' : `${offCount} off`}
            </p>
          </div>
          <RevealGroup as="ul" stagger={0.05} className="flex flex-col gap-2">
            {networkFeatures.map((f) => (
              <RevealItem as="li" key={f.id}>
                <Switch
                  checked={on[f.id] ?? true}
                  onChange={(v) => setOn((s) => ({ ...s, [f.id]: v }))}
                  label={<span className="font-medium">{f.name}</span>}
                  description={
                    on[f.id] ? (
                      <>
                        {f.endpoint} · sends: {f.sends}
                      </>
                    ) : (
                      'Off: never contacts the network'
                    )
                  }
                />
              </RevealItem>
            ))}
          </RevealGroup>
        </div>
      </div>
    </section>
  )
}
