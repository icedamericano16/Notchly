import { site } from './site'

export interface Faq {
  id: string
  question: string
  answer: string
}

export const faqs: Faq[] = [
  {
    id: 'free',
    question: 'Is Notchly free?',
    answer:
      'Yes. Every feature is included, with no licence key, no account and no telemetry. It is freeware: free to use, published on GitHub.',
  },
  {
    id: 'smartscreen',
    question: 'Is it safe? Why does Windows SmartScreen warn me?',
    answer:
      "SmartScreen warns about new apps that haven't yet built up download reputation with Microsoft. It doesn't mean anything harmful was found. Click More info, then Run anyway. Notchly runs entirely on your PC, its network features are all listed on the privacy page, and its code is on GitHub for anyone to read.",
  },
  {
    id: 'performance',
    question: 'Does it slow my PC down?',
    answer: `No. It's a single native Rust app of about ${site.facts.exeSize} with no browser engine inside. It uses 0% CPU while idle, draws on the GPU, and hides itself during fullscreen games and video.`,
  },
  {
    id: 'win10',
    question: 'Does it work on Windows 10?',
    answer:
      "Yes: Windows 10 version 2004 (build 19041) or later, and Windows 11, on 64-bit PCs. Liquid Glass falls back to simpler styles where Windows can't provide what it needs.",
  },
  {
    id: 'uninstall',
    question: 'How do I uninstall it?',
    answer:
      'Windows Settings › Apps › Installed apps › Notchly › Uninstall. The uninstaller offers to remove your Notchly data as well.',
  },
  {
    id: 'settings',
    question: 'Where are my settings stored?',
    answer:
      'In %APPDATA%\\Notchly (settings, themes, notes, to-dos and history). Logs are in %LOCALAPPDATA%\\Notchly\\logs. You can export and import settings from Settings › Privacy.',
  },
  {
    id: 'apple',
    question: 'Is Notchly affiliated with Apple?',
    answer: `No. ${site.disclaimer}`,
  },
]
