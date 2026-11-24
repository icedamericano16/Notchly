import { Music } from 'lucide-react'
import { AnimatePresence } from 'motion/react'
import * as m from 'motion/react-m'
import { useState, type CSSProperties } from 'react'
import { SectionHeading } from '@/components/common/SectionHeading'
import { Switch } from '@/components/common/Switch'
import { AlbumArt, Waveform } from '@/components/island/parts'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  customiseTabs,
  defaultTheme,
  materials,
  privacySignals,
  themePresets,
  type CustomiseTab,
  type Material,
  type PrivacySignal,
} from '@/content/customise'
import { cn } from '@/lib/cn'
import { islandSpring } from '@/lib/motion'

const signalColor: Record<PrivacySignal, string> = {
  mic: 'bg-live-orange',
  camera: 'bg-live-green',
  location: 'bg-brand-blue',
}

interface PreviewState {
  material: Material
  floating: boolean
  themeId: string
  signals: Record<PrivacySignal, boolean>
}

/** Tabs on the left change one aspect each; the preview on the right shows all of them live. */
export function Customise() {
  const [tab, setTab] = useState<CustomiseTab>('glass')
  const [state, setState] = useState<PreviewState>({
    material: 'liquid',
    floating: false,
    themeId: 'midnight',
    signals: { mic: true, camera: false, location: false },
  })
  const set = (patch: Partial<PreviewState>) => setState((s) => ({ ...s, ...patch }))

  return (
    <section aria-labelledby="customise-title" className="container-page pt-32 sm:pt-40">
      <SectionHeading
        id="customise-title"
        eyebrow="Make it yours"
        title="True black, or liquid glass."
        description="Pick a material, float it or fuse it to the edge, choose a theme. The preview updates as you go."
      />

      <Tabs
        value={tab}
        onValueChange={(v) => setTab(v as CustomiseTab)}
        className="mt-14 grid gap-8 lg:grid-cols-[1fr_1.35fr] lg:items-center"
      >
        <div className="flex flex-col gap-6">
          <TabsList
            aria-label="Customisation"
            className="h-auto w-full flex-wrap justify-start gap-1 rounded-full border border-hairline bg-surface-1 p-1 sm:w-fit"
          >
            {customiseTabs.map((t) => (
              <TabsTrigger
                key={t.id}
                value={t.id}
                className="h-9 flex-none rounded-full px-4 data-active:bg-surface-3 data-active:text-fg"
              >
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>

          {customiseTabs.map((t) => (
            <TabsContent key={t.id} value={t.id} className="flex flex-col gap-5">
              <div>
                <h3 className="text-2xl font-semibold tracking-tightish">{t.title}</h3>
                <p className="mt-2 leading-relaxed text-fg-muted">{t.body}</p>
              </div>
              {t.id === 'glass' && (
                <Segmented
                  label="Material"
                  options={materials}
                  value={state.material}
                  onChange={(material) => set({ material })}
                />
              )}
              {t.id === 'floating' && (
                <Segmented
                  label="Position"
                  options={[
                    { id: 'attached', label: 'Attached' },
                    { id: 'floating', label: 'Floating' },
                  ]}
                  value={state.floating ? 'floating' : 'attached'}
                  onChange={(v) => set({ floating: v === 'floating' })}
                />
              )}
              {t.id === 'themes' && (
                <div role="radiogroup" aria-label="Theme" className="flex flex-wrap gap-2">
                  {themePresets.map((p) => {
                    const checked = state.themeId === p.id
                    return (
                      <button
                        key={p.id}
                        type="button"
                        role="radio"
                        aria-checked={checked}
                        onClick={() => set({ themeId: p.id })}
                        className={cn(
                          'flex items-center gap-2 rounded-full border py-1.5 pr-3 pl-1.5 text-sm transition-colors',
                          checked
                            ? 'border-hairline-strong bg-surface-3 text-fg'
                            : 'border-hairline text-fg-muted hover:text-fg',
                        )}
                      >
                        <span
                          aria-hidden="true"
                          className="size-5 rounded-full"
                          style={{
                            backgroundImage: `linear-gradient(135deg, ${p.stops.join(', ')})`,
                          }}
                        />
                        {p.name}
                      </button>
                    )
                  })}
                </div>
              )}
              {t.id === 'privacy' && (
                <div className="flex flex-col gap-2">
                  {privacySignals.map((s) => (
                    <Switch
                      key={s.id}
                      label={`${s.label} in use by ${s.app}`}
                      checked={state.signals[s.id]}
                      dotClass={signalColor[s.id]}
                      onChange={(v) => set({ signals: { ...state.signals, [s.id]: v } })}
                    />
                  ))}
                </div>
              )}
            </TabsContent>
          ))}
        </div>

        <Preview state={state} />
      </Tabs>
    </section>
  )
}

function Preview({ state }: { state: PreviewState }) {
  const theme = themePresets.find((t) => t.id === state.themeId) ?? defaultTheme
  const accent = {
    '--island-a': theme.stops[0],
    '--island-b': theme.stops[1],
    '--island-c': theme.stops[2],
  } as CSSProperties
  const activeSignals = privacySignals.filter((s) => state.signals[s.id])

  const material = {
    solid: 'bg-island ring-1 ring-white/[0.06]',
    glass: 'bg-white/[0.12] ring-1 ring-white/25 backdrop-blur-xl backdrop-saturate-150',
    liquid:
      'bg-white/[0.06] ring-1 ring-white/30 backdrop-blur-md backdrop-saturate-200 backdrop-brightness-110 shadow-[inset_0_1px_0_rgb(255_255_255/0.5),inset_0_-8px_18px_rgb(255_255_255/0.08)]',
  }[state.material]

  return (
    <div
      role="img"
      aria-label={`Preview: ${state.material} material, ${state.floating ? 'floating' : 'attached'}, ${theme.name} theme${activeSignals.length ? `, ${activeSignals.map((s) => s.label.toLowerCase()).join(' and ')} in use` : ''}.`}
      className="relative aspect-[16/10] overflow-hidden rounded-card border border-hairline bg-wallpaper-busy shadow-island"
      style={accent}
    >
      <div className="absolute inset-x-0 top-0 h-px bg-white/20" />
      <m.div
        layout
        transition={islandSpring}
        className={cn(
          'absolute left-1/2 flex h-11 w-[min(72%,22rem)] -translate-x-1/2 items-center justify-between pr-4 pl-2 text-fg',
          material,
          state.floating
            ? 'top-4 rounded-full shadow-[0_18px_40px_-12px_rgb(0_0_0/0.8)]'
            : 'top-0 rounded-t-none rounded-b-[1.4rem]',
        )}
      >
        <div className="flex items-center gap-2">
          <AlbumArt className="size-7 rounded-lg">
            <Music className="relative size-3.5 text-canvas/70" />
          </AlbumArt>
          <span className="hidden text-[13px] sm:inline">Night Drive</span>
        </div>
        <div className="flex items-center gap-3">
          <Waveform />
          <div className="flex gap-1">
            <AnimatePresence>
              {activeSignals.map((s) => (
                <m.span
                  key={s.id}
                  initial={{ opacity: 0, scale: 0 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0 }}
                  transition={islandSpring}
                  className={cn('size-2 rounded-full', signalColor[s.id])}
                />
              ))}
            </AnimatePresence>
          </div>
        </div>
      </m.div>
      <p className="absolute right-4 bottom-3 font-mono text-[11px] text-white/70">
        {theme.name} · {materials.find((x) => x.id === state.material)?.label} ·{' '}
        {state.floating ? 'Floating' : 'Attached'}
      </p>
    </div>
  )
}

interface SegmentedProps<T extends string> {
  label: string
  options: { id: T; label: string }[]
  value: T
  onChange: (v: T) => void
}

function Segmented<T extends string>({ label, options, value, onChange }: SegmentedProps<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex w-fit rounded-full border border-hairline bg-surface-1 p-1"
    >
      {options.map((o) => {
        const checked = o.id === value
        return (
          <button
            key={o.id}
            type="button"
            role="radio"
            aria-checked={checked}
            onClick={() => onChange(o.id)}
            className={cn(
              'relative rounded-full px-4 py-1.5 text-sm transition-colors',
              checked ? 'text-canvas' : 'text-fg-muted hover:text-fg',
            )}
          >
            {checked && (
              <m.span
                layoutId={`seg-${label}`}
                transition={islandSpring}
                className="absolute inset-0 rounded-full bg-fg"
              />
            )}
            <span className="relative">{o.label}</span>
          </button>
        )
      })}
    </div>
  )
}
