import { Menu } from 'lucide-react'
import { useMotionValueEvent, useScroll } from 'motion/react'
import * as m from 'motion/react-m'
import { lazy, Suspense, useState } from 'react'
import { Link, NavLink } from 'react-router'
import { Button } from '@/components/ui/button'
import { primaryNav } from '@/content/site'
import { cn } from '@/lib/cn'
import { uiSpring } from '@/lib/motion'
import { isExternal } from '@/lib/utils'
import { WindowsIcon } from './BrandIcons'
import { GitHubStarBadge } from './GitHubStarBadge'
import { Logo } from './Logo'

const MobileMenu = lazy(() => import('./MobileMenu'))

/** Internal links shown in the centre; GitHub is the star badge on the right. */
const centreLinks = primaryNav.filter((l) => !isExternal(l.href))

export function Navbar() {
  const { scrollY } = useScroll()
  const [scrolled, setScrolled] = useState(false)
  useMotionValueEvent(scrollY, 'change', (y) => {
    const next = y > 16
    if (next !== scrolled) setScrolled(next)
  })

  return (
    <header className="fixed inset-x-0 top-0 z-50 pt-3">
      {/* Shrinks on scroll with transform + opacity only: the bar rises and tightens while a
          glass pill fades in behind it. */}
      <m.div
        className="container-page"
        initial={false}
        animate={{ y: scrolled ? -4 : 0 }}
        transition={uiSpring}
      >
        <nav aria-label="Main" className="relative flex h-14 items-center gap-2 px-3 sm:px-4">
          <m.div
            aria-hidden="true"
            className="absolute inset-0 rounded-full border border-hairline bg-surface-1/70 shadow-island backdrop-blur-glass backdrop-saturate-150"
            initial={false}
            animate={{ opacity: scrolled ? 1 : 0, scale: scrolled ? 1 : 1.03 }}
            transition={uiSpring}
          />

          <Link to="/" className="relative mr-auto rounded-full p-1" aria-label="Notchly home">
            <m.span
              className="flex origin-left"
              initial={false}
              animate={{ scale: scrolled ? 0.94 : 1 }}
              transition={uiSpring}
            >
              <Logo />
            </m.span>
          </Link>

          <DesktopLinks />

          <div className="relative ml-auto flex items-center gap-2">
            <GitHubStarBadge className="hidden sm:inline-flex" />
            <Button asChild size="lg" className="h-9 rounded-full px-4 font-medium">
              <Link to="/download">
                <WindowsIcon className="size-3.5" />
                <span className="hidden xl:inline">Download for Windows</span>
                <span className="xl:hidden">Download</span>
              </Link>
            </Button>
            <Suspense
              fallback={
                <Button
                  variant="ghost"
                  size="icon-lg"
                  className="rounded-full lg:hidden"
                  aria-label="Open menu"
                  disabled
                >
                  <Menu />
                </Button>
              }
            >
              <MobileMenu />
            </Suspense>
          </div>
        </nav>
      </m.div>
    </header>
  )
}

function DesktopLinks() {
  const [hovered, setHovered] = useState<string | null>(null)

  return (
    <ul
      // Centre links from lg up; below that they live in the menu (no overlap on tablets).
      className="absolute left-1/2 hidden -translate-x-1/2 items-center lg:flex"
      onMouseLeave={() => setHovered(null)}
    >
      {centreLinks.map((link) => (
        <li key={link.href} className="relative">
          <NavLink
            to={link.href}
            onMouseEnter={() => setHovered(link.href)}
            onFocus={() => setHovered(link.href)}
            onBlur={() => setHovered(null)}
            className={({ isActive }) =>
              cn(
                'relative block rounded-full px-3.5 py-2 text-sm transition-colors duration-200',
                isActive ? 'text-fg' : 'text-fg-muted hover:text-fg',
              )
            }
          >
            {({ isActive }) => (
              <>
                {/* One highlight that glides between links (shared layoutId). */}
                {hovered === link.href && (
                  <m.span
                    layoutId="nav-hover"
                    aria-hidden="true"
                    className="absolute inset-0 rounded-full bg-white/[0.06]"
                    transition={uiSpring}
                  />
                )}
                {isActive && (
                  <span
                    aria-hidden="true"
                    className="absolute bottom-1 left-1/2 h-px w-4 -translate-x-1/2 rounded-full bg-brand-gradient"
                  />
                )}
                <span className="relative">{link.label}</span>
              </>
            )}
          </NavLink>
        </li>
      ))}
    </ul>
  )
}
