import { useEffect, type MouseEvent } from 'react'
import { Outlet, useLocation, useNavigationType } from 'react-router'
import { Footer } from '@/components/common/Footer'
import { Grain } from '@/components/common/Grain'
import { Navbar } from '@/components/common/Navbar'
import { useFirstInteractionRoot } from '@/hooks/useFirstInteraction'
import { useLenis, useLenisRoot } from '@/hooks/useLenis'

/** New pages start at the top (or at their #hash); back/forward keeps the browser's place. */
function useScrollToTop() {
  const { pathname, hash } = useLocation()
  const navType = useNavigationType()
  const lenis = useLenis()
  useEffect(() => {
    if (navType === 'POP') return
    const target = hash ? document.getElementById(decodeURIComponent(hash.slice(1))) : null
    if (lenis) lenis.scrollTo(target ?? 0, { immediate: true, offset: target ? -96 : 0 })
    else if (target) target.scrollIntoView()
    else window.scrollTo(0, 0)
    // Only on navigation, not when Lenis mounts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathname, hash, navType])
}

export function RootLayout() {
  useFirstInteractionRoot()
  useLenisRoot()
  useScrollToTop()
  const lenis = useLenis()

  // Move focus (not just the viewport) to the content, with or without Lenis.
  const skipToContent = (e: MouseEvent<HTMLAnchorElement>) => {
    e.preventDefault()
    const main = document.getElementById('main')
    if (!main) return
    main.focus({ preventScroll: true })
    if (lenis) lenis.scrollTo(main, { immediate: true })
    else main.scrollIntoView()
  }

  return (
    <>
      <a
        href="#main"
        onClick={skipToContent}
        className="sr-only rounded-full bg-fg px-4 py-2 text-sm font-medium text-canvas focus:not-sr-only focus:fixed focus:top-4 focus:left-4 focus:z-[200]"
      >
        Skip to content
      </a>
      <Navbar />
      <main id="main" tabIndex={-1} className="outline-none">
        <Outlet />
      </main>
      <Footer />
      <Grain />
    </>
  )
}
