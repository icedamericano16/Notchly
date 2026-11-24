import { Slot } from 'radix-ui'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/cn'

interface ShimmerButtonProps extends ComponentProps<'button'> {
  asChild?: boolean
  size?: 'md' | 'lg'
}

/**
 * Primary CTA with a light sweep (after Magic UI's Shimmer Button): a white pill, a soft
 * accent glow, and a highlight that crosses it every few seconds. Transform-only animation.
 */
export function ShimmerButton({
  asChild = false,
  size = 'lg',
  className,
  children,
  ...props
}: ShimmerButtonProps) {
  const Comp = asChild ? Slot.Root : 'button'
  return (
    <Comp
      className={cn(
        'group relative inline-flex items-center justify-center gap-2 overflow-hidden rounded-full bg-fg font-medium text-canvas shadow-glow transition-[transform,box-shadow] duration-300 ease-out-quint outline-none select-none hover:-translate-y-px hover:shadow-[0_0_0_1px_rgb(255_255_255/0.12),0_16px_56px_-12px_rgb(255_255_255/0.32)] focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-ring active:translate-y-0 [&_svg]:shrink-0',
        size === 'lg'
          ? 'h-12 px-6 text-[15px] [&_svg]:size-4'
          : 'h-10 px-5 text-sm [&_svg]:size-3.5',
        className,
      )}
      {...props}
    >
      <Slot.Slottable>{children}</Slot.Slottable>
      <span
        aria-hidden="true"
        className="motion-deferred pointer-events-none absolute inset-y-0 left-0 w-1/3 animate-shimmer bg-gradient-to-r from-transparent via-black/[0.07] to-transparent"
      />
    </Comp>
  )
}
