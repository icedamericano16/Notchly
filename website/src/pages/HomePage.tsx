import { lazy, Suspense } from 'react'
import { JsonLd } from '@/components/common/JsonLd'
import { Seo } from '@/components/common/Seo'
import { Hero } from '@/components/sections/Hero'
import { TrustStrip } from '@/components/sections/TrustStrip'

// Below the fold: separate chunks, fetched right after the first paint.
const ScrollStory = lazy(() => import('@/components/sections/ScrollStory'))
const BelowFold = lazy(() => import('@/components/sections/BelowFold'))

export default function HomePage() {
  return (
    <>
      <Seo page="home" />
      <JsonLd />
      <Hero />
      <div className="mt-20 sm:mt-28">
        <TrustStrip />
      </div>
      <Suspense fallback={<div className="min-h-dvh" />}>
        <ScrollStory />
        <BelowFold />
      </Suspense>
    </>
  )
}
