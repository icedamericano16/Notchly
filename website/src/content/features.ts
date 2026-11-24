/**
 * The feature catalogue. Group names and items are from the product brief; one-line
 * descriptions follow the README, user guide and privacy policy.
 */

export type FeatureIcon =
  'activity' | 'sparkles' | 'calendar' | 'folder' | 'cpu' | 'palette' | 'code'

export interface FeatureItem {
  name: string
  description: string
}

export interface FeatureGroup {
  id: string
  title: string
  icon: FeatureIcon
  summary: string
  items: FeatureItem[]
}

export const featureGroups: FeatureGroup[] = [
  {
    id: 'live-activities',
    title: 'Live Activities',
    icon: 'activity',
    summary: "What's happening right now, living in the pill.",
    items: [
      {
        name: 'Music with synced lyrics',
        description:
          'Any player in the Windows media flyout. Album art, lyrics that follow the song (click a line to seek), and middle-click to play or pause.',
      },
      {
        name: 'Audio visualiser',
        description: 'A live visualiser that moves with whatever is playing.',
      },
      {
        name: 'Timers',
        description: 'Focus timer on Ctrl+Alt+F, or type “timer 12m” in Ask for a one-off.',
      },
      { name: 'Calls', description: 'Incoming calls take the pill, with accept and decline.' },
      { name: 'Downloads', description: 'Progress for downloads, right in the notch.' },
      {
        name: 'System changes',
        description:
          'Volume, brightness, battery, Wi-Fi, Caps/Num Lock and keyboard language pop up briefly and merge when they repeat.',
      },
      {
        name: 'Notifications',
        description: 'Mirrored Windows notifications, with an optional encrypted history.',
      },
      {
        name: 'Meeting mode',
        description:
          'During Teams, Zoom, Meet or Discord calls, the notch shows the call with mute and leave.',
      },
      {
        name: 'Phone Drop',
        description:
          'Send files between your phone and PC: scan a QR code on the same Wi-Fi and enter the PIN. LAN-only.',
      },
    ],
  },
  {
    id: 'smart-tools',
    title: 'Smart tools',
    icon: 'sparkles',
    summary: 'Ask, translate, read text off the screen and calculate, from the notch.',
    items: [
      {
        name: 'Ask with local AI',
        description:
          'Uses Ollama on your PC if installed, or your own Gemini, OpenAI or Anthropic key. Ctrl+Alt+Space.',
      },
      {
        name: 'Translate',
        description: 'Translate in Ask, or translate highlighted text anywhere with Ctrl+Alt+R.',
      },
      {
        name: 'OCR',
        description:
          'Drag a region or click a window and its text is copied, fully offline. Ctrl+Alt+T.',
      },
      { name: 'Maths', description: 'Type “=” in Ask and it does the sum.' },
      {
        name: 'Unit and currency converter',
        description: 'Units offline; currency with daily rates from Frankfurter.',
      },
    ],
  },
  {
    id: 'productivity',
    title: 'Productivity',
    icon: 'calendar',
    summary: 'Your day at a glance, one hover away.',
    items: [
      {
        name: 'Calendar (with Bikram Sambat)',
        description:
          'Google Calendar, Outlook or ICS links, with your next event in the notch and the Bikram Sambat date alongside.',
      },
      {
        name: 'To-do list',
        description:
          'Type naturally: “remind me at 5pm”, “tomorrow 9 standup”, “water plants every day 8am”.',
      },
      {
        name: 'Notes',
        description: 'First line is the title; “[ ]” makes a checklist item. Undo and redo.',
      },
      {
        name: 'Water tracker',
        description: 'Reminders spread over your active hours; tap Drank, Snooze or Skip.',
      },
      { name: 'Prayer times', description: 'For your city, set once or located from Windows.' },
      { name: 'Weather', description: 'Current weather and forecast from MET Norway.' },
      {
        name: 'Widgets',
        description: 'Pin clocks, weather and more to the desktop.',
      },
    ],
  },
  {
    id: 'files',
    title: 'Files & clipboard',
    icon: 'folder',
    summary: 'A pocket at the top of the screen for things in transit.',
    items: [
      {
        name: 'Shelf',
        description: 'Drop files, text or links on the notch; drag them back out into any app.',
      },
      {
        name: 'Clipboard history',
        description:
          'The last 50 items, with favourites (Ctrl+Alt+V). Password-manager copies are never stored.',
      },
      {
        name: 'Downloads manager',
        description:
          'Watches your Downloads folder and follows files as they grow, with speed and completion, from any browser.',
      },
      {
        name: 'Screenshots',
        description:
          'Capture, annotate (arrow, box, highlight, blur, text, crop) and copy. Ctrl+Alt+S.',
      },
    ],
  },
  {
    id: 'system',
    title: 'System',
    icon: 'cpu',
    summary: 'Know what your PC is doing, and who is using your camera.',
    items: [
      {
        name: 'System monitor',
        description:
          'CPU, RAM, GPU, disk and network at a glance, sampled only while you look at it.',
      },
      {
        name: 'Privacy dots',
        description:
          'Orange for microphone, green for camera, blue for location. See which app is using them, and close it.',
      },
    ],
  },
  {
    id: 'look',
    title: 'Look & feel',
    icon: 'palette',
    summary: 'True black by default; glass when you want it.',
    items: [
      {
        name: 'Liquid Glass and floating styles',
        description:
          'Solid true black, frosted Glass, or Liquid Glass that bends light at its edges. Or float the island below the screen edge.',
      },
      {
        name: 'Themes',
        description:
          'Built-in presets plus Theme Studio: edit, share as a code, follow Windows light/dark.',
      },
      {
        name: 'Smooth native animations',
        description: "Spring physics at your display's refresh rate, rendered on the GPU.",
      },
    ],
  },
  {
    id: 'developers',
    title: 'For developers',
    icon: 'code',
    summary: 'Put your own live activities in the notch.',
    items: [
      {
        name: 'Live Activities API',
        description:
          'Newline-delimited JSON over a per-user named pipe. Payloads are validated, never executed; each new app asks you first.',
      },
      { name: 'CLI', description: '`notchly live start | update | end` from any script.' },
      {
        name: 'Python and Node.js SDKs',
        description: 'Tiny, dependency-free wrappers around the pipe.',
      },
    ],
  },
]
