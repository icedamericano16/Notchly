import { ArrowUpRight, Lock } from 'lucide-react'
import { PageHeader } from '@/components/common/PageHeader'
import { Reveal, RevealGroup, RevealItem } from '@/components/common/Reveal'
import { Seo } from '@/components/common/Seo'
import { localData, networkFeatures, privacyIntro } from '@/content/privacy'
import { site } from '@/content/site'

export default function PrivacyPage() {
  return (
    <>
      <Seo page="privacy" />
      <PageHeader eyebrow="Privacy" title={privacyIntro.title} description={privacyIntro.body} />

      <div className="container-page flex flex-col gap-24">
        <section aria-labelledby="net-title">
          <h2 id="net-title" className="text-3xl text-display sm:text-4xl">
            Every network connection
          </h2>
          <p className="mt-3 max-w-2xl text-fg-muted">
            The complete list. Each one is a feature you can switch off in Settings › Privacy.
          </p>
          <Reveal className="mt-8 overflow-x-auto rounded-card border border-hairline bg-surface-1">
            <table className="w-full min-w-[40rem] text-left text-sm">
              <caption className="sr-only">Notchly's network connections</caption>
              <thead className="border-b border-hairline text-fg-muted">
                <tr>
                  <th scope="col" className="px-5 py-3 font-medium">
                    Feature
                  </th>
                  <th scope="col" className="px-5 py-3 font-medium">
                    Connects to
                  </th>
                  <th scope="col" className="px-5 py-3 font-medium">
                    What is sent
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-hairline">
                {networkFeatures.map((f) => (
                  <tr key={f.id}>
                    <th scope="row" className="px-5 py-4 font-medium">
                      {f.name}
                    </th>
                    <td className="px-5 py-4 text-fg-muted">{f.endpoint}</td>
                    <td className="px-5 py-4 text-fg-muted">{f.sends}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </Reveal>
          <p className="mt-4 text-sm text-fg-muted">{privacyIntro.note}</p>
        </section>

        <section aria-labelledby="local-title">
          <h2 id="local-title" className="text-3xl text-display sm:text-4xl">
            What stays on your PC
          </h2>
          <RevealGroup as="ul" className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {localData.map((d) => (
              <RevealItem
                as="li"
                key={d.title}
                className="rounded-card border border-hairline bg-surface-1 p-6"
              >
                <Lock className="size-4 text-live-green" aria-hidden="true" />
                <h3 className="mt-4 font-medium">{d.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-fg-muted">{d.body}</p>
              </RevealItem>
            ))}
          </RevealGroup>
        </section>

        <section aria-labelledby="cal-title" className="grid gap-8 lg:grid-cols-[1fr_2fr]">
          <h2 id="cal-title" className="text-3xl text-display sm:text-4xl">
            Calendar sign-in
          </h2>
          <Reveal className="flex flex-col gap-4 leading-relaxed text-fg-muted">
            <p>
              Connecting Google Calendar or Outlook is optional and happens in your own browser.
              Notchly asks only to read your calendar, can't change anything, and fetches events
              straight to your PC. There is no Notchly server.
            </p>
            <p>
              Notchly's use and transfer of information received from Google APIs adheres to the
              Google API Services User Data Policy, including the Limited Use requirements. Google
              user data is never transferred, sold, used for advertising or used to train AI models.
            </p>
          </Reveal>
        </section>

        <section className="flex flex-col items-start gap-4 rounded-card border border-hairline bg-surface-1 p-6 sm:flex-row sm:items-center sm:justify-between">
          <p className="text-fg-muted">
            This page is a summary. The full privacy policy has every detail.
          </p>
          <a
            href={`${site.repo.url}/blob/main/docs/PRIVACY.md`}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-fg px-4 py-2 text-sm font-medium text-canvas"
          >
            Read the full policy <ArrowUpRight className="size-4" aria-hidden="true" />
            <span className="sr-only"> (opens in a new tab)</span>
          </a>
        </section>
      </div>
    </>
  )
}
