import { Check, Copy } from 'lucide-react'
import { useEffect, useState } from 'react'
import { copyText } from '@/lib/clipboard'
import { cn } from '@/lib/cn'

interface CopyButtonProps {
  text: string
  label?: string
  className?: string
}

export function CopyButton({ text, label = 'Copy', className }: CopyButtonProps) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')

  useEffect(() => {
    if (state === 'idle') return
    const t = window.setTimeout(() => setState('idle'), 1800)
    return () => window.clearTimeout(t)
  }, [state])

  return (
    <button
      type="button"
      onClick={async () => setState((await copyText(text)) ? 'copied' : 'failed')}
      aria-label={state === 'copied' ? 'Copied' : label}
      className={cn(
        'flex size-8 items-center justify-center rounded-lg border border-hairline bg-surface-2 text-fg-muted transition-colors hover:border-hairline-strong hover:text-fg',
        className,
      )}
    >
      {state === 'copied' ? (
        <Check className="size-3.5 text-live-green" aria-hidden="true" />
      ) : (
        <Copy className="size-3.5" aria-hidden="true" />
      )}
      <span className="sr-only" aria-live="polite">
        {state === 'copied' ? 'Copied to clipboard' : state === 'failed' ? 'Copy failed' : ''}
      </span>
    </button>
  )
}
