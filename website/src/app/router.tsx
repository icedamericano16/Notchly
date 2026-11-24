import type { ComponentType } from 'react'
import { createBrowserRouter, type RouteObject } from 'react-router'
import HomePage from '@/pages/HomePage'
import { RootLayout } from './RootLayout'
import { RouteError } from './RouteError'

/** Lazy-load a page module whose default export is the page component. */
const page = (load: () => Promise<{ default: ComponentType }>) => async () => ({
  Component: (await load()).default,
})

const routes: RouteObject[] = [
  {
    path: '/',
    Component: RootLayout,
    ErrorBoundary: RouteError,
    HydrateFallback: () => null,
    children: [
      // The landing page is eager: it's the LCP route.
      { index: true, Component: HomePage },
      { path: 'features', lazy: page(() => import('@/pages/FeaturesPage')) },
      { path: 'download', lazy: page(() => import('@/pages/DownloadPage')) },
      { path: 'changelog', lazy: page(() => import('@/pages/ChangelogPage')) },
      { path: 'privacy', lazy: page(() => import('@/pages/PrivacyPage')) },
      { path: '*', lazy: page(() => import('@/pages/NotFoundPage')) },
    ],
  },
]

// Vite's BASE_URL is "/" (Vercel, Netlify) or "/Notchly/" (GitHub Pages).
const basename = import.meta.env.BASE_URL.replace(/\/$/, '') || '/'

export const router = createBrowserRouter(routes, { basename })
