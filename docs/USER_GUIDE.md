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

- **Control Center** (second page): Wi-Fi, Bluetooth, Airplane mode, Focus, Microphone,
  Camera and Dark mode switches; brightness and volume sliders (drag, scroll the mouse wheel
  or swipe on a trackpad over them); battery; quick actions including Keep awake.
  **In use** shows what's using your camera, microphone or location — tap it to see each app
  and close it (Force quit appears if it doesn't close). The Camera switch turns Windows'
  camera access for apps off; an app already using the camera may need closing.
- **Pages wrap around**: swipe sideways on a touchpad, Shift+scroll, or scroll over the page dots.
- **Music**: any player that shows in the Windows media flyout. Click the art for lyrics;
  click a lyric line to seek. The chip in the corner switches between players.
- **Focus timer**: Ctrl+Alt+F starts/pauses. Type `timer 12m` in Ask for a one-off timer.
- **Water**: reminders are spread over your active hours; tap Drank / Snooze / Skip. The bell on
  the Water page turns them on or off; long-press or right-click it to snooze or pause for today.
- **Notes**: the first line is the title, `[ ]` makes a checklist item, `- ` a bullet. Ctrl+Z /
  Ctrl+Y undo and redo; drag to select.
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

## Liquid Glass

Settings › Appearance › Material: **Solid** (the true-black notch), **Glass** (frosted: a strong
blur, tint and a lit rim) or **Liquid Glass** (glass that bends light at its edges and moves like
liquid).

- **Glass strength** goes from *Clearest* (see-through, the lens most visible) through *Default*
  to *Tinted*; it snaps at all three. The **Glass style** chips set everything at once: Crystal,
  Default, Frosted, Tinted, Obsidian.
- **Advanced glass options**: refraction (how much the rim bends what's behind), blur, rim light,
  motion response, where the tint comes from (theme, adaptive — album art while music plays,
  otherwise the colour behind the notch — or your own colour), light that follows the pointer,
  and a slow idle shimmer.
- Text stays readable over bright wallpapers on its own: the glass deepens its tint as needed.
- The status line under the presets says what shows and why. Liquid Glass falls back to Glass;
  both show **tinted glass** (no screen reading) on battery saver, with *Show Notchly in screen
  recordings* on, and while the screen can't be read (a secure desktop, a rotated display, a
  display driven by another GPU); Solid with transparency effects off, in High Contrast and over
  Remote Desktop. It comes back by itself.
- While Glass or Liquid Glass reads the screen, the notch doesn't appear in screenshots or
  screen shares — Windows keeps it out so the glass never sees itself. Turn on *Show Notchly in
  screen recordings* to show tinted glass instead.
- The space around the notch is click-through: you can click whatever is just outside it.

## Live Activities for developers

See the README. Example: `notchly live start --id dl --title "Download" --progress 0.3`.

## Troubleshooting

- *A shortcut doesn't work*: another app owns it — Notchly tells you; change it in Settings › Shortcuts.
- *No notification mirroring*: needs the Store build or the installer's "Enable notification mirroring"
  option, then allow Notchly in Windows Settings › Privacy › Notifications.
- *Notchly closed unexpectedly*: on the next start it offers to copy the crash report (it never
  leaves your PC on its own).
- *Notch hidden*: something is fullscreen, or quiet hours/focus is on (tray menu).
- *The notch is missing from a screenshot or screen share*: Glass and Liquid Glass keep it out
  of captures. Turn on *Show Notchly in screen recordings* (Settings › Appearance › Advanced glass
  options).
- *Liquid Glass shows "Glass", "Tinted glass" or "Solid"*: the status line under the presets says why
  (battery saver, transparency effects off, High Contrast, Remote Desktop ...).
- Logs: `%APPDATA%\Notchly\logs` (no personal content; Settings › About › *Your data* opens the
  folder). `NOTCHLY_TRACE=1` adds detail.
