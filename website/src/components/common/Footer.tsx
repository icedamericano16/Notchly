import { footerColumns, site } from '@/content/site'
import { Logo } from './Logo'
import { SmartLink } from './SmartLink'

export function Footer() {
  const year = new Date().getFullYear()

  return (
    <footer className="relative mt-32 overflow-hidden border-t border-hairline bg-canvas">
      <div className="container-page grid gap-12 py-16 md:grid-cols-[1.5fr_repeat(3,1fr)]">
        <div>
          <Logo />
          <p className="mt-4 max-w-xs text-sm leading-relaxed text-fg-muted">{site.tagline}</p>
          <p className="mt-6 font-mono text-xs text-fg-muted">
            {site.facts.platforms} · {site.facts.price} · Runs locally
          </p>
        </div>

        {footerColumns.map((col) => (
          <nav key={col.title} aria-labelledby={`footer-${col.title}`}>
            <h2 id={`footer-${col.title}`} className="text-sm font-medium text-fg">
              {col.title}
            </h2>
            <ul className="mt-4 flex flex-col gap-3">
              {col.links.map((link) => (
                <li key={link.label}>
                  <SmartLink
                    href={link.href}
                    className="text-sm text-fg-muted transition-colors duration-200 hover:text-fg"
                  >
                    {link.label}
                  </SmartLink>
                </li>
              ))}
            </ul>
          </nav>
        ))}
      </div>

      <div className="container-page">
        <div className="hairline-fade" />
      </div>

      <div className="container-page flex flex-col gap-3 py-8 text-xs leading-relaxed text-fg-muted md:flex-row md:items-start md:justify-between md:gap-12">
        <p className="shrink-0">
          © {year} {site.name} · Made by {site.author.name}
        </p>
        <p className="max-w-xl md:text-right">{site.disclaimer}</p>
      </div>

      {/* Oversized wordmark, cropped by the page edge. Decorative. */}
      <p
        aria-hidden="true"
        className="pointer-events-none -mb-[0.28em] bg-gradient-to-b from-white/[0.07] to-transparent bg-clip-text text-center text-[24vw] leading-none font-semibold tracking-display text-transparent select-none"
      >
        {site.name}
      </p>
    </footer>
  )
}
