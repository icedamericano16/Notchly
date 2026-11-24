import type { Transition, Variants } from 'motion/react'

/** The island spring: buttery, crisp, no wobble. */
export const islandSpring: Transition = { type: 'spring', stiffness: 400, damping: 30 }

/** A softer spring for UI chrome (navbar, pills). */
export const uiSpring: Transition = { type: 'spring', stiffness: 300, damping: 30 }

export const easeOutExpo = [0.16, 1, 0.3, 1] as const

/**
 * Scroll reveal: fade + rise + slight blur. With reduced motion, <MotionConfig
 * reducedMotion="user"> drops the transform, and `filter` is skipped below, so it becomes
 * a plain fade.
 */
export const revealVariants: Variants = {
  hidden: { opacity: 0, y: 12, filter: 'blur(6px)' },
  visible: {
    opacity: 1,
    y: 0,
    filter: 'blur(0px)',
    transition: { duration: 0.7, ease: easeOutExpo },
  },
}

export const reducedRevealVariants: Variants = {
  hidden: { opacity: 0 },
  visible: { opacity: 1, transition: { duration: 0.4 } },
}

export const staggerChildren = (stagger = 0.08, delay = 0): Variants => ({
  hidden: {},
  visible: { transition: { staggerChildren: stagger, delayChildren: delay } },
})
