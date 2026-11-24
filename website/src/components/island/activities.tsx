import {
  ArrowDown,
  Check,
  FileArchive,
  FolderOpen,
  MessageCircle,
  Music,
  Pause,
  Phone,
  PhoneOff,
  Play,
  Reply,
  SkipBack,
  SkipForward,
  Square,
  Timer,
  TriangleAlert,
} from 'lucide-react'
import { useState, type ComponentType } from 'react'
import { Link } from 'react-router'
import { islandDemo, type IslandActivityId } from '@/content/island'
import { formatClock, useDemoClock } from '@/hooks/useDemoClock'
import { AlbumArt, CompactRow, ProgressBar, RoundButton, Waveform } from './parts'

export type IslandSize = 'md' | 'sm'

interface Dims {
  w: number
  h: number
}

export interface ActivityView {
  /** Compact pill size per breakpoint. */
  size: Record<IslandSize, Dims>
  /** CSS colour of the soft glow under the island. */
  glow: string
  Compact: ComponentType
  Expanded: ComponentType
}

export const EXPANDED_WIDTH: Record<IslandSize, number> = { md: 400, sm: 320 }
export const COMPACT_HEIGHT: Record<IslandSize, number> = { md: 40, sm: 36 }

const dims = (md: number, sm: number): Record<IslandSize, Dims> => ({
  md: { w: md, h: COMPACT_HEIGHT.md },
  sm: { w: sm, h: COMPACT_HEIGHT.sm },
})

/* ─── Music ─────────────────────────────────────────────────────────────── */

const { music, timer, call, download, notification, notFound } = islandDemo

function useSongPosition() {
  const t = useDemoClock()
  const pos = t % music.durationSec
  const line = Math.floor(t / 3) % music.lyrics.length
  return { pos, line }
}

function MusicCompact() {
  return (
    <CompactRow
      left={
        <AlbumArt className="size-6">
          <Music className="relative size-3 text-canvas/70" />
        </AlbumArt>
      }
      right={<Waveform />}
    />
  )
}

function MusicExpanded() {
  const { pos, line } = useSongPosition()
  const [playing, setPlaying] = useState(true)
  const next = music.lyrics[(line + 1) % music.lyrics.length]

  return (
    <div className="flex flex-col gap-4 p-4 pr-12">
      <div className="flex items-center gap-3">
        <AlbumArt className="size-14 rounded-xl">
          <Music className="relative size-5 text-canvas/70" />
        </AlbumArt>
        <div className="min-w-0 flex-1">
          <p className="truncate text-[15px] font-medium">{music.title}</p>
          <p className="truncate text-[13px] text-fg-muted">{music.artist}</p>
        </div>
        <Waveform playing={playing} />
      </div>
      <div className="min-h-[2.75rem]">
        <p className="text-island-accent text-[17px] leading-snug font-semibold tracking-tightish">
          {music.lyrics[line]}
        </p>
        <p className="truncate text-[13px] text-fg-muted">{next}</p>
      </div>
      <div className="flex items-center gap-2 font-mono text-[11px] text-fg-muted tabular-nums">
        <span>{formatClock(pos)}</span>
        <ProgressBar value={pos / music.durationSec} />
        <span>{formatClock(music.durationSec)}</span>
      </div>
      <div className="flex items-center justify-center gap-4">
        <RoundButton label="Previous track" size="md">
          <SkipBack />
        </RoundButton>
        <RoundButton
          label={playing ? 'Pause' : 'Play'}
          tone="accent"
          size="lg"
          onClick={() => setPlaying((p) => !p)}
        >
          {playing ? <Pause className="fill-current" /> : <Play className="fill-current" />}
        </RoundButton>
        <RoundButton label="Next track" size="md">
          <SkipForward />
        </RoundButton>
      </div>
    </div>
  )
}

/* ─── Timer ─────────────────────────────────────────────────────────────── */

function useTimerLeft() {
  const t = useDemoClock()
  return timer.totalSec - 1 - (t % timer.totalSec)
}

function TimerCompact() {
  const left = useTimerLeft()
  return (
    <CompactRow
      left={
        <>
          <Timer className="size-4 text-live-orange" aria-hidden="true" />
          <span className="text-[13px] text-live-orange">{timer.label}</span>
        </>
      }
      right={
        <span className="font-mono text-[15px] font-medium text-live-orange tabular-nums">
          {formatClock(left)}
        </span>
      }
    />
  )
}

function TimerExpanded() {
  const live = useTimerLeft()
  const [pausedAt, setPausedAt] = useState<number | null>(null)
  const left = pausedAt ?? live

  return (
    <div className="flex items-center justify-between gap-4 p-5 pr-12">
      <div>
        <p className="flex items-center gap-1.5 text-[13px] text-live-orange">
          <Timer className="size-3.5" aria-hidden="true" />
          {timer.label} timer
        </p>
        <p className="mt-1 font-mono text-5xl font-light tracking-tight text-live-orange tabular-nums">
          {formatClock(left)}
        </p>
        <ProgressBar
          className="mt-3 w-40 [&>span]:bg-live-orange [&>span]:bg-none"
          value={1 - left / timer.totalSec}
        />
      </div>
      <div className="flex gap-2">
        <RoundButton
          label={pausedAt === null ? 'Pause timer' : 'Resume timer'}
          tone="orange"
          size="lg"
          onClick={() => setPausedAt((p) => (p === null ? live : null))}
        >
          {pausedAt === null ? (
            <Pause className="fill-current" />
          ) : (
            <Play className="fill-current" />
          )}
        </RoundButton>
        <RoundButton label="Stop timer" size="lg">
          <Square className="fill-current" />
        </RoundButton>
      </div>
    </div>
  )
}

/* ─── Call ──────────────────────────────────────────────────────────────── */

function Avatar({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={`flex shrink-0 items-center justify-center rounded-full bg-island-accent font-semibold text-canvas ${className ?? ''}`}
    >
      {call.initials}
    </span>
  )
}

function CallCompact() {
  return (
    <CompactRow
      left={
        <>
          <Avatar className="size-6 text-[11px]" />
          <span className="truncate text-[13px]">{call.name}</span>
        </>
      }
      right={
        <>
          <span className="flex size-6 items-center justify-center rounded-full bg-live-red">
            <PhoneOff className="size-3" aria-hidden="true" />
          </span>
          <span className="relative flex size-6 items-center justify-center rounded-full bg-live-green">
            <span className="absolute inset-0 animate-pulse-ring rounded-full bg-live-green" />
            <Phone className="relative size-3 text-canvas" aria-hidden="true" />
          </span>
        </>
      }
    />
  )
}

function CallExpanded() {
  const t = useDemoClock()
  const [answeredAt, setAnsweredAt] = useState<number | null>(null)
  const [declined, setDeclined] = useState(false)

  return (
    <div className="flex items-center justify-between gap-4 p-5 pr-12">
      <div className="flex min-w-0 items-center gap-3">
        <Avatar className="size-12 text-lg" />
        <div className="min-w-0">
          <p className="truncate text-[17px] font-medium">{call.name}</p>
          <p className="truncate text-[13px] text-fg-muted" aria-live="polite">
            {declined
              ? 'Call declined'
              : answeredAt !== null
                ? `On call · ${formatClock(t - answeredAt)}`
                : call.detail}
          </p>
        </div>
      </div>
      {!declined && (
        <div className="flex gap-3">
          <RoundButton
            label={answeredAt === null ? 'Decline' : 'End call'}
            tone="red"
            size="lg"
            onClick={() => setDeclined(true)}
          >
            <PhoneOff />
          </RoundButton>
          {answeredAt === null && (
            <RoundButton label="Accept" tone="green" size="lg" onClick={() => setAnsweredAt(t)}>
              <Phone />
            </RoundButton>
          )}
        </div>
      )}
    </div>
  )
}

/* ─── Download ──────────────────────────────────────────────────────────── */

function useDownloadProgress() {
  const t = useDemoClock()
  return ((t % download.cycleSec) + 1) / download.cycleSec
}

function DownloadCompact() {
  const p = useDownloadProgress()
  const r = 8
  const c = 2 * Math.PI * r
  return (
    <CompactRow
      left={
        <>
          <span className="flex size-6 items-center justify-center rounded-full bg-brand-blue/20">
            <ArrowDown className="size-3.5 text-brand-blue" aria-hidden="true" />
          </span>
          <span className="font-mono text-[13px] text-fg-muted tabular-nums">
            {Math.round(p * 100)}%
          </span>
        </>
      }
      right={
        <svg viewBox="0 0 20 20" className="size-5 -rotate-90" aria-hidden="true">
          <circle cx="10" cy="10" r={r} fill="none" strokeWidth="2.5" className="stroke-white/15" />
          <circle
            cx="10"
            cy="10"
            r={r}
            fill="none"
            strokeWidth="2.5"
            strokeLinecap="round"
            className="stroke-brand-blue transition-[stroke-dashoffset] duration-1000 ease-linear"
            strokeDasharray={c}
            strokeDashoffset={c * (1 - p)}
          />
        </svg>
      }
    />
  )
}

function DownloadExpanded() {
  const p = useDownloadProgress()
  const done = Math.round(p * download.totalMb)
  return (
    <div className="flex flex-col gap-3 p-5 pr-12">
      <div className="flex items-center gap-3">
        <span className="flex size-11 shrink-0 items-center justify-center rounded-xl bg-brand-blue/15">
          <FileArchive className="size-5 text-brand-blue" aria-hidden="true" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-[15px] font-medium">{download.file}</p>
          <p className="font-mono text-[12px] text-fg-muted tabular-nums">
            {done} of {download.totalMb} MB · {Math.round(p * 100)}%
          </p>
        </div>
      </div>
      <ProgressBar value={p} className="h-1.5 [&>span]:bg-brand-blue [&>span]:bg-none" />
      <div className="flex justify-end gap-2">
        <button
          type="button"
          className="inline-flex items-center gap-1.5 rounded-full bg-white/10 px-3 py-1.5 text-[12px] transition-colors hover:bg-white/15"
        >
          <FolderOpen className="size-3.5" aria-hidden="true" />
          Show in folder
        </button>
      </div>
    </div>
  )
}

/* ─── Notification ──────────────────────────────────────────────────────── */

function AppIcon({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={`flex shrink-0 items-center justify-center rounded-[28%] bg-live-green ${className ?? ''}`}
    >
      <MessageCircle className="size-[55%] fill-fg text-fg" />
    </span>
  )
}

function NotificationCompact() {
  return (
    <CompactRow
      left={
        <>
          <AppIcon className="size-6" />
          <span className="truncate text-[13px]">
            <span className="font-medium">{notification.sender}</span>
            <span className="text-fg-muted"> · {notification.text}</span>
          </span>
        </>
      }
      right={<span className="text-[12px] text-fg-muted">now</span>}
    />
  )
}

function NotificationExpanded() {
  const [read, setRead] = useState(false)
  return (
    <div className="flex flex-col gap-3 p-5 pr-12">
      <div className="flex items-start gap-3">
        <AppIcon className="size-10" />
        <div className="min-w-0">
          <p className="text-[12px] text-fg-muted">{notification.app} · now</p>
          <p className="text-[15px] font-medium">{notification.sender}</p>
          <p className="text-[14px] leading-snug text-fg/85">{notification.text}</p>
        </div>
      </div>
      <div className="flex gap-2 pl-[3.25rem]">
        <button
          type="button"
          className="inline-flex items-center gap-1.5 rounded-full bg-white/10 px-3 py-1.5 text-[12px] transition-colors hover:bg-white/15"
        >
          <Reply className="size-3.5" aria-hidden="true" />
          Reply
        </button>
        <button
          type="button"
          onClick={() => setRead(true)}
          aria-pressed={read}
          className="inline-flex items-center gap-1.5 rounded-full bg-white/10 px-3 py-1.5 text-[12px] transition-colors hover:bg-white/15"
        >
          <Check className="size-3.5" aria-hidden="true" />
          {read ? 'Read' : 'Mark as read'}
        </button>
      </div>
    </div>
  )
}

/* ─── Not found (404 page) ──────────────────────────────────────────────── */

function NotFoundCompact() {
  return (
    <CompactRow
      left={
        <>
          <TriangleAlert className="size-4 text-live-orange" aria-hidden="true" />
          <span className="truncate text-[13px]">{notFound.title}</span>
        </>
      }
      right={<span className="font-mono text-[13px] text-live-orange">{notFound.code}</span>}
    />
  )
}

function NotFoundExpanded() {
  return (
    <div className="flex items-center justify-between gap-4 p-5 pr-12">
      <div>
        <p className="font-mono text-[12px] text-live-orange">Error {notFound.code}</p>
        <p className="text-[17px] font-medium">{notFound.title}</p>
        <p className="text-[13px] text-fg-muted">This page doesn't exist.</p>
      </div>
      <Link
        to="/"
        className="rounded-full bg-fg px-4 py-2 text-[13px] font-medium text-canvas transition-colors hover:bg-fg/90"
      >
        Go home
      </Link>
    </div>
  )
}

/* ─── Idle ──────────────────────────────────────────────────────────────── */

function IdleCompact() {
  return null
}

function IdleExpanded() {
  return (
    <div className="p-5 pr-12">
      <p className="text-[13px] text-fg-muted">Nothing playing</p>
      <p className="text-[17px] font-medium">All quiet.</p>
    </div>
  )
}

/* ─── Registry ──────────────────────────────────────────────────────────── */

export const activities: Record<IslandActivityId, ActivityView> = {
  idle: {
    size: dims(124, 112),
    glow: 'transparent',
    Compact: IdleCompact,
    Expanded: IdleExpanded,
  },
  music: {
    size: dims(296, 252),
    glow: 'var(--color-brand-violet)',
    Compact: MusicCompact,
    Expanded: MusicExpanded,
  },
  timer: {
    size: dims(252, 224),
    glow: 'var(--color-live-orange)',
    Compact: TimerCompact,
    Expanded: TimerExpanded,
  },
  call: {
    size: dims(280, 244),
    glow: 'var(--color-live-green)',
    Compact: CallCompact,
    Expanded: CallExpanded,
  },
  download: {
    size: dims(264, 232),
    glow: 'var(--color-brand-blue)',
    Compact: DownloadCompact,
    Expanded: DownloadExpanded,
  },
  notification: {
    size: dims(340, 292),
    glow: 'var(--color-live-green)',
    Compact: NotificationCompact,
    Expanded: NotificationExpanded,
  },
  notFound: {
    size: dims(260, 232),
    glow: 'var(--color-live-orange)',
    Compact: NotFoundCompact,
    Expanded: NotFoundExpanded,
  },
}
