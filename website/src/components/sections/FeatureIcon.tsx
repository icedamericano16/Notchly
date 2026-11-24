import {
  Activity,
  CalendarDays,
  Code2,
  Cpu,
  FolderClosed,
  Palette,
  Sparkles,
  type LucideIcon,
} from 'lucide-react'
import type { FeatureIcon as FeatureIconId } from '@/content/features'
import { cn } from '@/lib/cn'

const featureIcons: Record<FeatureIconId, LucideIcon> = {
  activity: Activity,
  sparkles: Sparkles,
  calendar: CalendarDays,
  folder: FolderClosed,
  cpu: Cpu,
  palette: Palette,
  code: Code2,
}

export function FeatureIcon({ id, className }: { id: FeatureIconId; className?: string }) {
  const Icon = featureIcons[id]
  return (
    <span
      className={cn(
        'flex size-9 items-center justify-center rounded-xl border border-hairline bg-surface-2 text-fg',
        className,
      )}
    >
      <Icon className="size-4" aria-hidden="true" />
    </span>
  )
}
