/*
 * BlurText — after React Bits' BlurText (reactbits.dev, MIT): words fade in from a blur as
 * they scroll into view. Uses Motion's light `m` components; with reduced motion the words
 * simply fade.
 */
import * as m from 'motion/react-m'
import type { ElementType } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { easeOutExpo } from '@/lib/motion'

interface BlurTextProps {
  text: string
  as?: ElementType
  className?: string
  /** Seconds between words. */
  stagger?: number
  delay?: number
}

export function BlurText({
  text,
  as: Tag = 'p',
  className,
  stagger = 0.05,
  delay = 0,
}: BlurTextProps) {
  const reduced = useReducedMotion()
  const words = text.split(' ')

  return (
    <Tag className={className}>
      <m.span
        className="inline"
        initial="hidden"
        whileInView="visible"
        viewport={{ once: true, margin: '0px 0px -10% 0px' }}
        transition={{ staggerChildren: stagger, delayChildren: delay }}
      >
        {words.map((word, i) => (
          <span key={`${word}-${i}`}>
            <m.span
              className="inline-block"
              variants={
                reduced
                  ? { hidden: { opacity: 0 }, visible: { opacity: 1 } }
                  : {
                      hidden: { opacity: 0, y: 10, filter: 'blur(8px)' },
                      visible: { opacity: 1, y: 0, filter: 'blur(0px)' },
                    }
              }
              transition={{ duration: 0.6, ease: easeOutExpo }}
            >
              {word}
            </m.span>
            {i < words.length - 1 && ' '}
          </span>
        ))}
      </m.span>
    </Tag>
  )
}
