import { cn } from '@/lib/cn'
import { CopyButton } from './CopyButton'

interface CodeBlockPlainProps {
  code: string
  className?: string
  label?: string
}

/** Un-highlighted code with a copy button (for short shell snippets). */
export function CodeBlockPlain({ code, className, label = 'Copy commands' }: CodeBlockPlainProps) {
  return (
    <div className={cn('relative rounded-xl border border-hairline bg-canvas', className)}>
      <pre className="overflow-x-auto p-4 pr-12 font-mono text-[13px] leading-relaxed text-fg/90">
        <code>{code}</code>
      </pre>
      <CopyButton text={code} label={label} className="absolute top-2.5 right-2.5" />
    </div>
  )
}
