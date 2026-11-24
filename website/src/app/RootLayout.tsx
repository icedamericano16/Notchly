import type { MouseEvent } from 'react'
import { Outlet, ScrollRestoration } from 'react-router'
import { Footer } from '@/components/common/Footer'
import { Grain } from '@/components/common/Grain'
import { Navbar } from '@/components/common/Navbar'
import { useLenis, useLenisRoot } from '@/hooks/useLenis'

export function RootLayout() {
  useLenisRoot()
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
      <ScrollRestoration />
    </>
  )
}
