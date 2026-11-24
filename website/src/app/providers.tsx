import { LazyMotion, MotionConfig } from 'motion/react'
import type { ReactNode } from 'react'
import { HelmetProvider } from 'react-helmet-async'
import { TooltipProvider } from '@/components/ui/tooltip'

interface ProvidersProps {
  children: ReactNode
}

const loadMotionFeatures = () => import('@/lib/motion-features').then((m) => m.default)

export function Providers({ children }: ProvidersProps) {
  return (
    <HelmetProvider>
      {/* `strict`: components must use the light `m.*` (from motion/react-m), never
          `motion.*`, or the full engine would land back in the entry chunk. */}
      <LazyMotion features={loadMotionFeatures} strict>
        {/* "user": with prefers-reduced-motion, Motion skips transform animations and
            keeps opacity, which is exactly the fallback the design asks for. */}
        <MotionConfig reducedMotion="user">
          <TooltipProvider delayDuration={200}>{children}</TooltipProvider>
        </MotionConfig>
      </LazyMotion>
    </HelmetProvider>
  )
}
