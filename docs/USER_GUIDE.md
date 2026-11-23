# Notchly user guide

## The basics

- **Hover** the notch to open it (or **click**, if you chose click mode). Move away to close.
- **Win+Alt+N** opens and closes it from the keyboard. Inside: ←/→ switch pages, Tab/↑/↓ move
  focus, Enter/Space activates, Esc closes, Ctrl+P pins it open, 1–9 jump to a page.
- **Right-click** the notch for the menu (pin an activity, settings, quit). **Middle-click**
  plays/pauses. **Scroll** over the collapsed notch changes volume (Shift+scroll seeks).
- The dots under a page switch pages; the pin keeps the notch open, × closes it.

## What shows in the collapsed notch

The most important live activity owns the pill (calls › finishing timers › recording/OCR ›
focus timer › Live API › downloads › music › charging › next event › prayer). A second activity
appears as a bubble — click it to swap. Pop-ups (volume, battery, Wi-Fi…) show briefly and merge
when they repeat. Fullscreen games, video and slideshows hide the notch automatically.

## Features

- **Control Center** (second page): Wi-Fi, Bluetooth, Focus, Dark mode, Keep awake and mic
  switches, brightness and volume sliders (drag up/down), battery, and quick actions.
- **Pages wrap around**: swipe sideways on a touchpad, Shift+scroll, or scroll over the page dots.
- **Music**: any player that shows in the Windows media flyout. Click the art for lyrics;
  click a lyric line to seek. The chip in the corner switches between players.
- **Focus timer**: Ctrl+Alt+F starts/pauses. Type `timer 12m` in Ask for a one-off timer.
- **Water**: reminders are spread over your active hours; tap Drank / Snooze / Skip.
- **To-do**: type naturally — `call mom 6pm`, `tomorrow 9 standup`, `dentist fri 14:30`,
  `water plants every day 8am`, `check oven in 20 min`.
- **Shelf**: drop files, text or links on the notch; drag them back out into any app.
- **Clipboard** (Ctrl+Alt+V): last 50 items; pin favourites. Password-manager copies are never stored.
- **Smart OCR** (Ctrl+Alt+T): drag a region or click a window; text is copied, fully offline.
- **Screenshot** (Ctrl+Alt+S): capture, annotate (arrow, box, highlight, blur, text, crop), copy.
- **Ask** (Ctrl+Alt+Space): uses Ollama on this PC if installed (`ollama pull llama3.2`), or your own
  Gemini/OpenAI/Anthropic key. With neither, Ask shows *Get Ollama* and *Add an API key* buttons. `=` does maths, `>` runs commands, Tab switches to Translate.
  **Translate selection** (Ctrl+Alt+R) translates highlighted text anywhere.
- **Weather / prayer times**: set your city in Settings › Calendar & weather, or use *Locate* and
  turn on *Keep location updated* to refresh it from Windows at every start.
- **Calendar**: paste ICS links, or click *Connect* next to Google Calendar or Outlook and sign in
  in your browser. (Builds without built-in sign-in let you enter your own OAuth client ID.)
- **Privacy dots**: orange mic, green camera, blue location. The Privacy page lists recent use.
- **Meeting mode**: during Teams/Zoom/Meet/Discord calls, the notch shows the call with mute/leave.
- **Phone Drop**: Shelf › QR button, scan with your phone on the same Wi-Fi, enter the PIN.
- **Widgets**: search "Desktop widgets" in the palette to pin clocks, weather and more to the desktop.
- **Notifications**: mirrored Windows toasts and their history. Turn on *Keep notification history*
  (Settings › Pop-ups) to keep it between restarts; it's stored encrypted for your Windows account.
- **Quiet hours**: *Allow incoming calls* and *Allow timers* decide what still gets through.
- **Music**: set a *Preferred player* (e.g. Spotify) to favour it when several apps play.

## Live Activities for developers

See the README. Example: `notchly live start --id dl --title "Download" --progress 0.3`.

## Troubleshooting

- *A shortcut doesn't work*: another app owns it — Notchly tells you; change it in Settings › Shortcuts.
- *No notification mirroring*: needs the Store build or the installer's "Enable notification mirroring"
  option, then allow Notchly in Windows Settings › Privacy › Notifications.
- *Notchly closed unexpectedly*: on the next start it offers to copy the crash report (it never
  leaves your PC on its own).
- *Notch hidden*: something is fullscreen, or quiet hours/focus is on (tray menu).
- Logs: `%APPDATA%\Notchly\logs` (no personal content; Settings › About › *Your data* opens the
  folder). `NOTCHLY_TRACE=1` adds detail.
