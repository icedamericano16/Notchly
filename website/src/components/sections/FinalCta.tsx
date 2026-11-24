import { DownloadButton } from '@/components/common/DownloadButton'
import { Reveal } from '@/components/common/Reveal'
import { LogoMark } from '@/components/common/Logo'
import { Magnet } from '@/components/reactbits/Magnet'
import { finalCta } from '@/content/home'

export function FinalCta() {
  return (
    <section aria-labelledby="cta-title" className="container-page pt-32 sm:pt-40">
      <Reveal className="relative isolate overflow-hidden rounded-frame border border-hairline px-6 py-16 text-center sm:px-12 sm:py-24">
        {/* Gradient field: static layers, no animation. */}
        <div aria-hidden="true" className="absolute inset-0 -z-10 bg-surface-1" />
        <div
          aria-hidden="true"
          className="absolute -top-1/2 left-1/2 -z-10 h-[140%] w-[120%] -translate-x-1/2 rounded-full bg-[radial-gradient(closest-side,rgb(91_108_255/0.35),transparent)]"
        />
        <div
          aria-hidden="true"
          className="absolute inset-x-0 top-0 -z-10 h-px bg-brand-gradient opacity-60"
        />

        <LogoMark className="mx-auto h-10" />
        <h2 id="cta-title" className="mx-auto mt-8 max-w-3xl text-4xl text-display sm:text-6xl">
          {finalCta.title}
        </h2>
        <p className="mx-auto mt-5 max-w-xl text-lg text-fg-muted">{finalCta.body}</p>
        <div className="mt-10 flex justify-center">
          <Magnet padding={40} strength={6}>
            <DownloadButton showMeta />
          </Magnet>
        </div>
      </Reveal>
    </section>
  )
}
