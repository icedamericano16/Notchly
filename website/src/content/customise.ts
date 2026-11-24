/** Customisation showcase. Theme stops are the app's real presets (src/theme/mod.rs). */

export type CustomiseTab = 'glass' | 'floating' | 'themes' | 'privacy'

export const customiseTabs: { id: CustomiseTab; label: string; title: string; body: string }[] = [
  {
    id: 'glass',
    label: 'Liquid Glass',
    title: 'Solid, Glass or Liquid Glass',
    body: 'True black by default. Glass is frosted with a lit rim; Liquid Glass bends the light at its edges and moves like liquid. Text stays readable over any wallpaper: the glass deepens its tint when it has to.',
  },
  {
    id: 'floating',
    label: 'Floating',
    title: 'Attached or floating',
    body: 'Keep the notch fused to the top edge like hardware, or let it float just below as an island with its own shadow.',
  },
  {
    id: 'themes',
    label: 'Themes',
    title: 'Themes, or make your own',
    body: 'Built-in presets cross-fade perceptually so colours never pass through grey. Theme Studio edits any of them, checks contrast, and shares themes as a code.',
  },
  {
    id: 'privacy',
    label: 'Privacy dots',
    title: 'Know when you are being heard or seen',
    body: 'An orange dot while an app uses the microphone, green for the camera, blue for location. Open Control Center › In use to see which app, and close it.',
  },
]

export type Material = 'solid' | 'glass' | 'liquid'

export const materials: { id: Material; label: string }[] = [
  { id: 'solid', label: 'Solid' },
  { id: 'glass', label: 'Glass' },
  { id: 'liquid', label: 'Liquid Glass' },
]

export interface ThemePreset {
  id: string
  name: string
  stops: readonly [string, string, string]
}

export const defaultTheme: ThemePreset = {
  id: 'midnight',
  name: 'Midnight',
  stops: ['#19B8FF', '#2D5BFF', '#7B4BFF'],
}

export const themePresets: ThemePreset[] = [
  defaultTheme,
  { id: 'aurora', name: 'Aurora', stops: ['#2EE6C5', '#19B8FF', '#4D7CFF'] },
  { id: 'sunset', name: 'Sunset', stops: ['#FFB547', '#FF5E7E', '#B34BFF'] },
  { id: 'rose', name: 'Rose', stops: ['#FF8FB1', '#FF4F8B', '#C13CFF'] },
  { id: 'ocean', name: 'Ocean', stops: ['#64D2FF', '#0A84FF', '#5E5CE6'] },
  { id: 'mint', name: 'Mint', stops: ['#63E6BE', '#30D158', '#00C7BE'] },
  { id: 'mono', name: 'Mono', stops: ['#FFFFFF', '#C9CED9', '#8C94A8'] },
  { id: 'ember', name: 'Ember Glass', stops: ['#FFC15E', '#FF7A59', '#F0456E'] },
]

export type PrivacySignal = 'mic' | 'camera' | 'location'

export const privacySignals: { id: PrivacySignal; label: string; app: string }[] = [
  { id: 'mic', label: 'Microphone', app: 'Voice Recorder' },
  { id: 'camera', label: 'Camera', app: 'Camera' },
  { id: 'location', label: 'Location', app: 'Maps' },
]
