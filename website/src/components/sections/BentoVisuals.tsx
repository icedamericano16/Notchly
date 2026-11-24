/* The pictures in the bento cards. Every app value in them is a real capture of Notchly
   (content/media.ts); the theme swatches are the app's real presets and the code is the
   real CLI. */
import { CaptureImage } from '@/components/common/CaptureImage'
import { DesktopPreview } from '@/components/common/DesktopPreview'
import { themePresets } from '@/content/customise'
import { captures, type AppCapture } from '@/content/media'

/** A real page of the app, on the neutral desktop, at most its on-screen size. */
function Capture({ capture }: { capture: AppCapture }) {
  return (
    <div className="flex h-44 items-start justify-center overflow-hidden rounded-xl bg-desktop px-3 pt-0 sm:h-48">
      <CaptureImage capture={capture} />
    </div>
  )
}

export function LiveVisual() {
  return (
    <DesktopPreview
      captures={[
        captures.pillMusic,
        captures.music,
        captures.pillTimer,
        captures.popupDownload,
        captures.timerNotification,
      ]}
      interval={2.6}
      zoom={2.4}
      minWidth={48}
      label={null}
      className="aspect-auto h-44 rounded-xl sm:h-48"
    />
  )
}

export function AskVisual() {
  return <Capture capture={captures.ask} />
}

export function TodoVisual() {
  return <Capture capture={captures.todo} />
}

export function ShelfVisual() {
  return <Capture capture={captures.shelf} />
}

export function SystemVisual() {
  return <Capture capture={captures.sysmon} />
}

export function ThemeVisual() {
  return (
    <div
      aria-hidden="true"
      className="flex h-44 flex-wrap content-center items-center justify-center gap-2.5 rounded-xl bg-surface-2 p-4 sm:h-48"
    >
      {themePresets.map((t, i) => (
        <span
          key={t.id}
          className="size-8 animate-float rounded-full ring-1 ring-white/10"
          style={{
            backgroundImage: `linear-gradient(135deg, ${t.stops[0]}, ${t.stops[1]}, ${t.stops[2]})`,
            animationDelay: `${i * 0.25}s`,
            animationDuration: '3s',
          }}
        />
      ))}
    </div>
  )
}

export function CodeVisual() {
  return (
    <div aria-hidden="true" className="flex h-44 items-center rounded-xl bg-surface-2 sm:h-48">
      <pre className="w-full [scrollbar-width:thin] overflow-x-auto p-4 font-mono text-[12px] leading-relaxed text-fg-muted">
        <span className="text-brand-indigo">notchly</span> live start --id build-42 --title Build
        {'\n'}
        <span className="text-brand-indigo">notchly</span> live update --id build-42 --progress 0.8
        {'\n'}
        <span className="text-brand-indigo">notchly</span> live end --id build-42 --final{' '}
        <span className="text-live-green">"Done in 2m 14s"</span>
      </pre>
    </div>
  )
}
