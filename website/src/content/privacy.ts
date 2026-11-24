/** From docs/PRIVACY.md ("Network connections (complete list)"). */

export interface NetworkFeature {
  id: string
  name: string
  endpoint: string
  sends: string
}

export const networkFeatures: NetworkFeature[] = [
  {
    id: 'lyrics',
    name: 'Synced lyrics',
    endpoint: 'lrclib.net (then lyrics.ovh)',
    sends: 'Song title, artist, album, duration',
  },
  {
    id: 'currency',
    name: 'Currency rates',
    endpoint: 'api.frankfurter.app',
    sends: 'Nothing (daily rates)',
  },
  {
    id: 'weather',
    name: 'Weather',
    endpoint: 'api.met.no',
    sends: 'Coordinates rounded to 2 decimals',
  },
  {
    id: 'city',
    name: 'City search',
    endpoint: 'open-meteo.com geocoding',
    sends: 'The city name you type',
  },
  {
    id: 'calendar',
    name: 'Calendar',
    endpoint: 'Google, Microsoft Graph, your ICS links',
    sends: 'Your sign-in token, to read your events',
  },
  {
    id: 'ask',
    name: 'Ask / Translate',
    endpoint: 'Ollama on this PC, or your own Gemini / OpenAI / Anthropic key',
    sends: 'Your prompt and what you attach',
  },
  {
    id: 'updates',
    name: 'Update check',
    endpoint: 'GitHub Releases (signed manifest)',
    sends: 'Nothing (at most daily)',
  },
  {
    id: 'phone-drop',
    name: 'Phone Drop',
    endpoint: 'Your local network only',
    sends: 'Files you choose',
  },
]

export const privacyIntro = {
  eyebrow: 'Privacy',
  title: 'Everything runs locally.',
  body: 'No account, no telemetry, no analytics. Nothing you copy, type, hear or see leaves your PC unless you use a feature that obviously needs the network, and every one of those can be switched off in Settings › Privacy.',
  note: 'Every request uses HTTPS (except Ollama on localhost and Phone Drop on your LAN), identifies itself as Notchly/<version>, and times out after 10 seconds.',
}

export const localData = [
  {
    title: 'Stored on your PC',
    body: 'Settings, themes, to-dos, notes, clipboard history and caches live in %APPDATA%\\Notchly. Logs without personal content are in %LOCALAPPDATA%\\Notchly\\logs.',
  },
  {
    title: 'Secrets are encrypted',
    body: 'API keys are encrypted with Windows DPAPI. Calendar tokens live in Windows Credential Manager. Notification history is off by default, and encrypted when on.',
  },
  {
    title: 'Glass never leaves the GPU',
    body: 'Glass reads the screen behind the notch into a GPU texture. Those pixels are never copied to memory, written to disk or sent anywhere.',
  },
  {
    title: 'Indicators read names, not data',
    body: 'Privacy dots ask Windows which apps use the camera, mic or location. Only app names are read, never audio, video or positions.',
  },
  {
    title: 'The keyboard hook is narrow',
    body: 'It only reacts to Caps Lock, Num Lock and language-switch keys, and never records keystrokes.',
  },
  {
    title: 'Uninstall removes it all',
    body: 'The uninstaller offers to delete every file Notchly created.',
  },
]
