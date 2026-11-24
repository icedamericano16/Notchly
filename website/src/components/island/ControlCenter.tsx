import { Bluetooth, Camera, Moon, Wifi, type LucideIcon } from 'lucide-react'
import { useState } from 'react'
import { controlTiles, type ControlTileId } from '@/content/island'
import { cn } from '@/lib/cn'

const icons: Record<ControlTileId, LucideIcon> = {
  wifi: Wifi,
  bluetooth: Bluetooth,
  camera: Camera,
  focus: Moon,
}

/** The quick switches row of the real Control Center page, as working toggles. */
export function ControlCenter() {
  const [on, setOn] = useState<Record<ControlTileId, boolean>>(
    () =>
      Object.fromEntries(controlTiles.map((t) => [t.id, t.on])) as Record<ControlTileId, boolean>,
  )

  return (
    <div className="border-t border-white/[0.06] px-4 pt-3 pb-4">
      <p className="mb-2 text-[11px] font-medium tracking-wide text-fg-muted uppercase">
        Control Center
      </p>
      <div className="grid grid-cols-4 gap-2">
        {controlTiles.map((tile) => {
          const Icon = icons[tile.id]
          const active = on[tile.id]
          return (
            <button
              key={tile.id}
              type="button"
              aria-pressed={active}
              onClick={() => setOn((s) => ({ ...s, [tile.id]: !s[tile.id] }))}
              className={cn(
                'flex flex-col items-center gap-1.5 rounded-2xl py-2.5 text-[11px] transition-colors duration-200',
                active ? 'bg-white/[0.12] text-fg' : 'bg-white/[0.05] text-fg-muted hover:text-fg',
              )}
            >
              <span
                className={cn(
                  'flex size-7 items-center justify-center rounded-full transition-colors duration-200',
                  active ? 'bg-island-accent text-canvas' : 'bg-white/10',
                )}
              >
                <Icon className="size-3.5" aria-hidden="true" />
              </span>
              {tile.label}
            </button>
          )
        })}
      </div>
    </div>
  )
}
