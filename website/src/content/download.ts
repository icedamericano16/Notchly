import { site } from './site'

/**
 * Which release asset is which. Patterns are tried in order against asset file names;
 * change them here if the release naming changes.
 */
export const assetPatterns = {
  /** Inno Setup installer, e.g. Notchly-0.3.0-setup.exe. Preferred download. */
  installer: [/setup.*\.exe$/i, /installer.*\.exe$/i],
  /** MSIX package, offered as a secondary option. */
  msix: [/\.msixbundle$/i, /\.msix$/i],
  /** The bare exe, used only when no installer is attached. */
  portable: [/^notchly.*\.exe$/i],
} as const

export type AssetKind = keyof typeof assetPatterns

export const requirements = [
  { label: 'Windows', value: 'Windows 10 version 2004 (build 19041) or later, or Windows 11' },
  { label: 'Processor', value: '64-bit (x64)' },
  { label: 'Graphics', value: 'Any GPU with DirectX 11 (Direct2D / DirectComposition)' },
  { label: 'Disk', value: `About ${site.facts.exeSize}` },
  { label: 'Permissions', value: 'No administrator rights needed: installs for your account only' },
] as const

export interface InstallStep {
  title: string
  body: string
}

export const installSteps: InstallStep[] = [
  {
    title: 'Run the installer',
    body: 'Open the downloaded Notchly setup file. It installs for your account only, with no administrator prompt, and takes a few seconds.',
  },
  {
    title: 'If SmartScreen appears, click More info → Run anyway',
    body: "Windows shows this for new apps until they build up download reputation. It doesn't mean anything was detected. Notchly runs fully on your PC, and its code is public on GitHub.",
  },
  {
    title: 'Hover the notch',
    body: 'A true-black notch appears at the top-center of your screen. Hover it to open the control center. Win+Alt+N opens it from the keyboard.',
  },
]

export const buildFromSource = {
  requirements: [
    'Rust stable with the MSVC toolchain (rustup)',
    'Windows 10 SDK',
    'Visual Studio 2022 Build Tools with "Desktop development with C++"',
  ],
  commands: `git clone ${site.repo.url}.git
cd ${site.repo.name}
cargo build --release
# → target\\release\\notchly.exe`,
}

export const uninstallSteps = [
  'Open Windows Settings › Apps › Installed apps.',
  'Find Notchly, click ⋯ › Uninstall.',
  'The uninstaller offers to delete your Notchly data too (settings, notes, history in %APPDATA%\\Notchly). Choose No to keep it for a later reinstall.',
]

/** Copy for the non-Windows path ("Send link to my PC"). */
export const otherOsCopy = {
  title: 'Notchly is for Windows 10/11',
  body: "You're on another system right now. Copy the link and open it on your Windows PC.",
  button: 'Send link to my PC',
  copied: 'Link copied',
}
