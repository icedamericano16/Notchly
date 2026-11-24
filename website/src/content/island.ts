/**
 * Demo data for the React island replica. It's illustrative (a made-up song, caller and
 * file), shown in states the real app has: now playing with synced lyrics, timers, calls,
 * downloads and notifications.
 */

export type IslandActivityId =
  'idle' | 'music' | 'timer' | 'call' | 'download' | 'notification' | 'notFound'

export interface IslandActivityMeta {
  /** Short name for controls ("Now playing"). */
  label: string
  /** What a screen reader hears for the current state. */
  description: string
}

export const islandDemo = {
  music: {
    title: 'Night Drive',
    artist: 'Demo Artist',
    durationSec: 214,
    lyrics: [
      'City lights are fading slow',
      'Keep the radio down low',
      'Every road a brand-new line',
      'Hold the moment, take your time',
    ],
  },
  timer: { label: 'Focus', totalSec: 25 * 60 },
  call: { name: 'Alex', initials: 'A', detail: 'Incoming call · mobile' },
  download: { file: 'project-assets.zip', totalMb: 248, cycleSec: 18 },
  notification: {
    app: 'Messages',
    sender: 'Sam',
    text: 'Lunch at 1? I booked the usual place.',
  },
  notFound: { title: 'Page not found', code: '404' },
} as const

export const islandMeta: Record<IslandActivityId, IslandActivityMeta> = {
  idle: { label: 'Idle', description: 'Collapsed notch, nothing happening.' },
  music: {
    label: 'Now playing',
    description: `Now playing ${islandDemo.music.title} by ${islandDemo.music.artist}, with synced lyrics.`,
  },
  timer: { label: 'Timer', description: 'A focus timer counting down.' },
  call: { label: 'Call', description: `Incoming call from ${islandDemo.call.name}.` },
  download: {
    label: 'Download',
    description: `Downloading ${islandDemo.download.file}.`,
  },
  notification: {
    label: 'Notification',
    description: `New message from ${islandDemo.notification.sender}.`,
  },
  notFound: { label: 'Not found', description: 'Page not found, error 404.' },
}

/** The hero's cycle, in the order the brief asks for. */
export const heroSequence: IslandActivityId[] = [
  'music',
  'timer',
  'call',
  'download',
  'notification',
]

/** Control Center tiles shown when the island expands (all real Control Center switches). */
export const controlTiles = [
  { id: 'wifi', label: 'Wi-Fi', on: true },
  { id: 'bluetooth', label: 'Bluetooth', on: true },
  { id: 'camera', label: 'Camera', on: true },
  { id: 'focus', label: 'Focus', on: false },
] as const

export type ControlTileId = (typeof controlTiles)[number]['id']
