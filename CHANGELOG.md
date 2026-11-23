# Changelog

## 0.3.0 "Fluid" — in progress

### Fixed: the grey box around the notch
- With *Glass* (and whenever Liquid Glass fell back to Windows' blur), a flat grey rectangle the
  size of the whole notch window showed around the island. Windows' blur ignores the window's
  shape, so Notchly no longer uses it: every material is now drawn by Notchly itself and the
  notch is exactly its own shape — shoulders, corners, shadow — in every state and size.
- **Glass** is now Notchly's own frosted glass (strong blur, tint, lit rim, no lens), calm and
  light on power. **Liquid Glass** falls back to Glass, then to **tinted glass** (no screen
  reading at all), then Solid; the status line in Settings says which one shows and why.
- The empty space around the notch no longer catches the mouse: clicks and hovers just outside
  the island reach the app underneath.
- The floating shadow and the glow pulses were cut off at the window edge; they now fade out
  fully.
- Switching material cross-fades instead of snapping.
- Fixed in the same round: the notch briefly stopped opening on hover or click (it wasn't
  receiving the mouse after the click-through change).
- `--demo` adds a pass through every material, style and view.

### Glass and Liquid Glass look different
- They used to blur and tint almost the same. **Glass** is now frosted acrylic: a heavy blur,
  a milky haze, fine grain, muted colour and a soft, even rim; its light stays still.
  **Liquid Glass** is clear: a lighter blur so the colours behind read through, a stronger
  lens that visibly bends what's behind at the rim, vivid colour, crisp highlights on two
  opposite edges, and light that follows the pointer.

### Shelf: a soft landing
- Dropped files no longer hit a hard floor and bounce fast. A card now drops onto a soft
  cushion: it falls, presses in and squashes softly, rises once in a slow, low rebound and
  settles (about 1.2 s). A contact shadow gathers under it as it lands, the surface ripples
  outward in a flat ring and a few soft puffs drift aside — no shockwaves or sparks.

### Idle pill
- The time sits exactly in the middle of the notch. Headphones / Bluetooth / offline go on
  its left, charging / low battery and the privacy dots on its right, and the pill grows
  evenly so nothing ever overlaps the time. The minute's digits roll up as it changes.

### Control Center
- **Scroll or swipe on the brightness and volume sliders.** A mouse wheel moves them in clean
  5 % steps that glide; a trackpad follows your fingers 1:1. Pushing past the top or bottom
  stretches the slider elastically and it springs back. No volume or brightness pop-up covers
  Control Center while you adjust.
- **Airplane mode** replaces Keep awake in the switches (all radios off, and back as they
  were). Keep awake moved to the quick actions (the cup; it glows amber with a wisp of steam
  while it holds the PC awake).
- **Camera**: turns Windows' camera access for apps off and on (the lens closes behind a
  shutter when blocked). If an app already has the camera, the sheet below opens so you can
  close it.
- **In use**: a new tile shows what's using your camera, microphone or location, with a
  breathing dot for each. Tap it for **In use now**: every app with its capability and for how
  long, and **Close app** (asks the app to close, so it can save) — if it's still there after
  3 seconds, **Force quit**. Windows' own processes are never offered. The Privacy page has
  the same buttons. Esc or a click outside closes the sheet.
- The Microphone and Camera tiles say who is using them ("Live · Zoom", "In use · Teams").
- Control Center is laid out on one grid: the switches 2 × 4 on the left; on the right,
  brightness and volume as shorter capsules beside a battery card (charge ring, charging /
  time left, opens battery settings), and six labelled quick-action tiles below.

### Fixed: "Nothing is using your camera, microphone or location" during calls
- Newer Windows builds (25H2 / 26H2) stopped recording camera and microphone use where
  Notchly looked (the consent store's usage times), so a WhatsApp video call showed nothing.
  Notchly now asks Windows directly: the audio engine for every app recording from a
  microphone, and Media Foundation's sensor activity monitor for every app streaming a camera.
  Both update within a second and need no admin rights. Apps built on Edge WebView2
  (WhatsApp, for one) are credited to the app itself, by its real name, and Close closes it.
  Location still comes from the consent store, which these Windows builds no longer update.
- `notchly --privacy-check` prints what's in use and which sources are live.

## 0.2.0 — 2026-11-23 — Liquid Glass

### Liquid Glass
- A new material, **Liquid Glass**: the notch is a thick piece of optical glass over your
  desktop. What's behind it is blurred, bent at the rim like light through the edge of a lens,
  lifted in colour (saturation up, highlights tamed) and tinted; the glass has a soft inner
  shadow, a light top edge, a specular rim lit from the top-left and a sheen. Edges are
  anti-aliased and follow the notch exactly, shoulders included.
- **One strength slider** from Clearest through Default to Tinted (with detents at all three),
  plus presets: **Crystal**, **Default**, **Frosted**, **Tinted**, **Obsidian**. *Advanced glass
  options* adds refraction, blur, rim light, motion response, the tint source (theme, adaptive
  or your own colour), light that follows the pointer and an optional idle shimmer.
- A **live preview** at the top of Settings draws the real glass over a sample wallpaper, and a
  status line says which glass is active and why it fell back (battery saver, transparency off,
  High Contrast, Remote Desktop ...).
- **Glass on glass**: cards, tiles, inputs and controls are thinner glass with their own rim and
  top light instead of solid plates. Solid looks exactly as before.
- **Legibility first**: Notchly measures how bright it is behind the notch and deepens the tint
  just enough to keep text at 4.5:1 contrast or better; secondary text brightens with it.
- **Glass that moves like liquid**: it stretches slightly along the way it springs, the lens
  swells with speed, presses flash light from the press point, the light drifts toward the
  pointer, pop-ups and Live Activities arrive with a ripple, and the dual island's bridge is one
  continuous piece of glass. Reduce motion turns all of it off.
- **Glass** (Windows' own blur) is better too: its window region now follows the notch outline
  every frame (no jagged corners, no lag behind the animation), it's dark like the rest of
  Notchly, and it gets the rim, depth and a soft stand-in for the lens.
- **Six glass-first themes**: Crystal, Pearl, Sapphire, Aurora Glass, Ember Glass, Obsidian.
  Every theme and theme pack gets matching glass; packs may set `"glass": {"tint", "rim",
  "sheen", "shadow", "glow"}`. Theme switches cross-fade.
- **Type and icons**: Segoe UI Variable through its real weight and optical-size axes (titles at
  590, captions at 510), pixel-snapped icons that stay crisp at small sizes, icon strokes that
  match the weight of the text beside them, layered glass glyphs in page headers and the dock,
  and new micro-animations: the bell swings when a notification arrives, play and pause morph,
  Wi-Fi bars fill in, volume waves spread out.

### Notes
- Liquid Glass reads the pixels behind the notch on the GPU to draw the glass. They never leave
  the GPU and are never saved, logged or sent (see PRIVACY.md). Because of this, Windows keeps
  the notch out of screen captures, recordings and shares while Liquid Glass is on; turn on
  *Show Notchly in screen recordings* to use Windows' blur instead.
- Older settings move over automatically: *Glass* with its intensity becomes Liquid Glass at a
  matching strength.
- `notchly --perf` reports the active glass, backdrop refreshes per second and GPU time per frame.
- `--demo` ends with a glass showcase (presets, themes and both tiers), then restores your look.

## 0.1.9 — 2026-11-23

### Notes, redesigned
- A wider two-pane page: a sidebar of note cards beside a full editor. Each note keeps its own
  colour (card tab, a tinted wash on the page, the caret); cards show the title, a preview,
  when it was edited, a pin and checklist progress (ring + "2/4"). Pinned notes come first and
  the selection glides between cards.
- **Search notes** filters as you type; Enter opens the first match.
- **Live-preview editor**: the first line reads as a bold title, `[ ]` / `[x]` lines are real
  checkboxes you can tick with a click (even while typing; the tick draws in and the item is
  struck through), `- ` lines become coloured bullets. The text stays plain, so sync and
  Markdown export are unchanged.
- Exact caret, selection highlight and click-to-place (wrapped lines included); Home / End and
  Up / Down move by line; Enter continues a checklist or bullet list and ends it on an empty
  item; the checklist button turns the current line into a checklist item; long notes scroll
  with the wheel, follow the caret and fade at the edge.
- Footer: when it was edited, word count, a checklist progress bar and a "Saved" check that pops
  in once you stop typing.
- Header: app chip, note count, tools (checklist, pin, export, delete) and an accent **New**
  button. An illustrated empty state (two drifting sheets) when there are no notes.

### Fixes
- After a restart the editor could open blank on the last note, and typing then replaced that
  note's text. The editor now always mirrors the open note.

## 0.1.8 — 2026-11-23

### Home, redesigned
- The music and battery tiles are gone. The left side is a living clock card: its colours follow
  the time of day (night, dawn, day, dusk), the sun (or moon) travels a small arc at its real
  height for your location, stars twinkle at night, the colon breathes with the seconds and a
  hairline fills each minute; date, Bikram Sambat, holiday chip and world-clock pills; a glance
  row with the current weather and the next sunrise / sunset.
- Right side: the calendar week card and a new **Notifications** card with the notifications
  that matter most first (calls, then people and reminders, then the rest; system chatter last),
  newest first within each, a count badge, *Clear all*, a dismiss button on each, and "+N more".
- Tap a notification to read it in full on the Notifications page (it glides open, with
  *Dismiss*); dismiss any row with its ×. The Notifications page is sorted the same way.
- Settings: the Home tile pickers are gone with the tiles.

### Notifications now work without the installer
- When the notification listener API isn't available (no package identity, as when running the
  exe directly), Notchly reads the Action Center's own notification store read-only, so your real
  Windows notifications appear in Notchly, live. App names come from Windows itself
  (e.g. "PredatorSense", "Snipping Tool", "WhatsApp"). Documented in the privacy policy.

### Shelf drops
- Dropped files now fall with real gravity and bounce down to rest (three shrinking hops, a squash
  on every landing, a slight stretch while falling), instead of one springy bounce that looked
  stuck. The drop waits until the shelf is on screen, so the whole fall is visible; the ripple and
  sparks fire on the first landing.
## 0.1.7 — 2026-11-23

### Lyrics timing
- **Auto-sync to the music.** Videos often start with a silent intro or logo, and browsers
  report "playing" before the audio really starts (buffering), so the player's clock ran ahead
  and lyrics appeared before the singer. Notchly now listens to the speaker level from the top
  of each track; the first sustained sound marks where the song really begins, and the lyrics
  wait that long (capped at 12 s, longer only when the video is much longer than the matched
  recording). If the player later corrects its own clock, the delay is taken back out so it's
  never applied twice. The lyrics toolbar shows "Syncing to the music…" then "Auto-synced +2.3s".
- The −/+ nudge now goes the right way (+ = lyrics later) and stacks on top of auto-sync;
  clicking a line seeks to where it's actually sung.
- Lines appear a touch closer to the voice (180 ms lead, the word sweep 60 ms).

### Fixed
- The speaker level meter was never connected, so "Equaliser source: Audio" was quietly
  simulated. A light meter thread (only while something plays) now feeds the equaliser and
  auto-sync. `notchly --meter-check` reports whether it can read your speakers.
## 0.1.6 — 2026-11-23

### Lyrics, rebuilt
- **Finds lyrics for YouTube and browser videos.** Titles like "Song - Artist (Lyrics)" uploaded
  by a channel are split both ways and cleaned of "(Official Video)", "| Lyrics", "ft. …",
  "slowed + reverb", hashtags and similar noise; LRCLIB is asked with exact lookups and free-text
  search, every hit is scored by title/artist word overlap and duration (videos run long), and
  synced lyrics win. lyrics.ovh is the fallback for plain lyrics.
- Old "not found" results are no longer cached forever (retried after 3 days; the cache was reset).
- When a match identifies the song, the player shows its real title and artist
  ("Dilruba Dilnasheen · Rito Riba · 7clouds" instead of the raw video title).
- `notchly --lyrics-check "title" "artist" seconds` shows the candidates and the match.

### Music + lyrics on one page
- The separate Lyrics page is gone; a **CC button** on the player shows or hides lyrics
  (also in Settings › Music). With lyrics on, the page grows into a compact player above a
  lyrics stage washed in the album art's colours with drifting aurora light.
- Synced lyrics: the current line lights up word by word with a feathered edge and a soft glow,
  neighbours shrink and fade with distance, long lines fit, interludes and intros show three
  breathing dots that fill toward the next line, click any line to jump there.
- Tools: source badge (LRCLIB · Synced), translation, a timing nudge (−/+ 0.25 s, remembered
  per song, click the value to reset), *Try the next match* when there are several, and web
  search. Plain lyrics scroll with the song; loading shows a shimmer; misses explain what was
  searched with *Try again* and *Search the web*.
## 0.1.5 — 2026-11-23

### Control Center
- Switches are now wide two-line tiles: an icon disc, the name and a live status — network name
  with signal bars, headset name with battery, how long Keep awake has been on, Focus holding pop-ups.
- Each switch has its own animation: Wi-Fi arcs light up from the dot outward (a slash draws when
  off), the Bluetooth rune draws itself and pulses, the Focus moon swings in with twinkling stars,
  the Dark mode disc turns over, steam curls from the Keep awake cup, the microphone shows level
  bars going live and a slash with a shake when muted. Every switch ripples and pops.

### Calendar
- Google: every calendar you show in Google Calendar (primary, holidays, shared) is read, each in
  its own colour; all-day events keep their full day; cancelled events are skipped.
- Looks 30 days ahead (was 7).
- The Agenda no longer asks you to connect when you already have: it shows the connected accounts
  and "synced N min ago", a spinning refresh while syncing, and a proper "You're all clear" state
  with *Open Google Calendar*. Sync problems show the real error.
- `notchly --calendar-check` prints connection status and event counts for troubleshooting;
  the log records counts (never titles).

### To-do
- Date & time picker beside the field: Today / Tomorrow / This weekend / Next week, a month grid,
  time chips and Once / Daily / Weekly. The pick shows as a chip in the field and wins over a time
  written in the text.

### Focus timer (redesigned)
- Gradient ring with minute ticks that light up, a glowing head, a phase halo; Focus / Short /
  Long mode switch with a sliding gradient pill; big play button; custom lengths; session dots
  that fill as the session runs; breaks breathe ("Breathe in / Breathe out"); paused time pulses.

### Weather
- A live sky behind current conditions: day / night gradients, sun rays, drifting clouds, rain,
  snow, lightning, fog and twinkling stars, with city, wind and data age.
- Hover the 24-hour chart for that hour's temperature, conditions, rain chance, wind and humidity.
- Hover the sun path to read the sun's elevation at any time of day.

### Audit
- No mock data reaches the app: the only hard-coded samples are the debug QA script, `--demo`
  and unit tests. System, weather, calendar, prayer and battery values are all live.
## 0.1.4 — 2026-11-23

### New
- **Control Center page** (second in the carousel): Wi-Fi and Bluetooth radio switches,
  Focus, Dark mode (apps + taskbar), Keep awake, microphone mute; tall iOS-style brightness and
  volume sliders; battery ring; quick actions (screenshot, capture text, lock, night light,
  settings, sound settings). Toggles fill with their colour on a bouncy spring and pop when switched.
- **Wrap-around paging**: after the last page comes the first (and back), with the slide following
  the direction of travel. Touchpad swipes and the wheel over the page dots change pages; travel
  is accumulated and debounced so one flick is one page.
- **Weather is now "today in detail"**: current conditions with high/low, feels-like, wind and
  humidity; a **live sun path** that plots the sun's real elevation through the day for your
  location, with the sun drawn where it is right now (moon at night), sunrise/sunset, daylight
  length and time left; a 24-hour temperature curve with conditions, rain bars and "Now".
  Solar position is computed offline (NOAA algorithm, tested against known sunrise times).
- **Home calendar card** replaces the timer and water tiles: month, a week strip with today lit
  and event dots, and the next event with "in 25m". Toggle in Settings › Dashboard & widgets.
- **To-do redesign**: All / Today / Upcoming / Done tabs with a sliding pill and counts, a live
  preview of the time it understood while you type, card rows with coloured due chips (overdue,
  today, later), a green check-off with pop, snooze on hover, *Clear completed*, today's progress ring.
- **Music**: One UI-style wavy seek bar that travels while playing and eases flat on pause.

### Polish
- Water page: water-tinted glass with rising bubbles, percentage, "ml to go" / goal reached,
  this week's bars.
- Night forecasts show the moon; the weather refresh button no longer overlaps the forecast.
- The Widgets page left the notch carousel (desktop widgets stay in Settings).
- "Take a screenshot" in the command palette now takes a screenshot (it opened text capture).
- Hindi and Nepali for everything new.
## 0.1.3 — 2026-11-23

### Finished features
- **Settings that did nothing now work**: *Keep notification history* (saved encrypted with DPAPI,
  deleted when turned off), *City search* privacy switch, *Preferred player*, *Allow incoming calls*
  and *Allow timers* in quiet hours, *Keep location updated* (refresh from Windows at start-up).
- **Calendar**: one-click Google / Outlook sign-in when the build includes the publisher's OAuth
  clients; your own client ID (and Google secret) still works. Sign-in results show as pop-ups and
  events refresh straight away.
- **Ask**: the unavailable "Windows AI" option is gone (old settings fall back to Auto); with no
  provider, Ask offers *Get Ollama* and *Add an API key*.
- **Updates**: the feed is set at build time; *Check now* says "You're up to date", "Couldn't check…"
  or "Downloading…"; a verified update shows an *Install & restart* pop-up.
- **Smart OCR**: "No OCR language" offers *Add language*.
- **Crash notice**: after an unexpected exit, the next start offers *Copy report* and *Report* /
  *Open folder*. Reports never leave the PC on their own.
- **Phone Drop**: stops after repeated wrong PINs; constant-time PIN check; slowed guessing.
- **Notification mirroring via the installer**: the exe manifest now names the sparse identity
  package (it never attached before), and the updater no longer mistakes that identity for a Store install.

### Premium polish
- Notifications, notes, agenda, privacy log and downloads get the shared motion: rows spring in,
  slide out with a cascade and collapse, and lift on hover. Notes get a selection pill that glides
  between rows; notifications get tinted app avatars; events in progress glow.
- Live-activity and download progress bars glide to new values; unknown-size downloads shimmer.
- Hindi and Nepali now cover 570 strings: page text, empty states, settings descriptions,
  pop-ups, tooltips and accessible names. Only product names and shortcuts stay in English.
- Sharper app icon and generated Store / tile logos.

### Release engineering
- `packaging\build.ps1`: one command for a signed exe, sparse identity package, Store MSIX,
  installer and `update.json` (PFX, self-signed test or Azure Trusted Signing). CI uses it on tags.
- Fixed the Store manifest (capability order failed schema validation).
- Freeware licence, third-party notices, git repository, version 0.1.3.

## 0.1.2 — 2026-11-23

### Notch
- Minimal idle mode is now progressive: the hairline gets a wide invisible hover band, the pill
  (time + status) appears instantly on hover, the full notch opens after a ~450 ms dwell, and the
  pill lingers 2.5 s after the pointer leaves or the notch collapses. Fast pointer passes wait for
  the pointer to settle instead of cancelling.
- Hovering the page dots grows the notch 24 DIP at the bottom so the icon dock sits below the page
  content instead of covering it; the notch shrinks back when the pointer leaves the dock.

### Shelf & clipboard
- Delete = thrown in the trash: the item lifts, folds into a chip and flies along an arc (tilting,
  red wash) into a trash can that springs up in the corner; the lid opens as it approaches, the can
  gulps with squash-and-wobble, puffs dust, and springs away. "Clear all" / "Clear" throw items one
  after another (whole volley ≤ 0.45 s).
- Drop on the shelf: the notch jiggles like jelly, new cards fall in from the notch, stretch while
  falling and squash on landing, with shockwave rings and a spray of sparks.
- Debug QA: `NOTCHLY_QA=1 notchly --page shelf --qa-trash` plays a scripted drop → delete → clear all.

## 0.1.1 — 2026-11-23

### Design
- iOS-style springs (response / damping-fraction tuned: island 0.40 s @ 0.80, snappy 0.24 s @ 0.92,
  bouncy 0.38 s @ 0.62, gentle 0.50 s critically damped).
- Apple dark palette (neutral greys, system colours), per-feature tints on tiles, headers and the dock;
  new themes Blossom, Ocean and Mint.
- Idle notch shows the time and status glyphs (headphones, Bluetooth, charging %, low battery, offline)
  that pop in on the bouncy spring; cross-fades with the Blink mascot.
- Hovering the page dots morphs them into an icon dock (active page tinted, hover springs, settings).
- Redesigned System page: animated ring gauges for CPU / memory / GPU, gradient area charts for disk
  and network.

### Shelf, clipboard, to-do
- Items spring in when dropped / copied / added and animate out on delete (scale-fade on the shelf,
  slide-and-collapse in lists); neighbours glide into place.
- Shelf: delete button on every card, "Clear all"; clipboard: "Clear" (keeps pinned); to-do drag-to-reorder.

### Completed features
- Screenshot annotation drawing (arrow, rectangle, highlighter, blur, crop, typed text labels).
- "Find city" now applies the location (weather + prayer times update immediately).
- Local QR-code decoding in Smart OCR (works even without an OCR language installed).
- Settings: dashboard tile picker, countdown title/date, desktop-widget toggles, idle clock toggle.
- Wired previously inert settings: automatic lyrics translation, Do Not Disturb during focus,
  quiet-hours "allow timers", live high-CPU alert toggle.
- Widgets page in the dock; `--page <name> [--dock]` QA flag; `NOTCHLY_QA=1` (debug) side-by-side instance.

## 0.1.0 — 2026-11-23

First complete build. Free for everyone.

### Core island
- DirectComposition notch with concave shoulders and continuous-curvature corners; click-through
  by resizing the window to the pill every frame; device-loss recovery; per-monitor DPI v2.
- Spring library (island, snappy, bouncy, gentle; calm/fast; reduced motion) and named choreography:
  width-then-height expand, 55 % content reveal with 25 ms stagger, fade-then-collapse, page slides,
  activity roll, pop-up arrival/merge/exit, notification drop-in with type-on, glow pulses,
  charging sweep, low-battery breathing, gooey dual-island split, shared-element album art,
  spring hover/press on every button and tile.
- Frame scheduler: vblank while animating, ~30 fps for live visuals, 1 Hz tickers, idle otherwise.
- Activity Manager with priorities, dual island, pinning, swap; pop-up queue (merge, stale drop, preempt).
- Fullscreen / presentation suppression, quiet hours, focus, pause pop-ups, digest afterwards.

### Features
- Music (all sessions, picker, seek, shuffle/repeat, per-app volume, title clean-up), synced lyrics
  with karaoke sweep and cache, lyrics-in-live-view, translation/romanisation.
- Pop-ups: volume, brightness, caps/num lock, input language, battery, network, audio device,
  Bluetooth (+ headset battery), mic mute.
- Dashboard tiles, focus timer, water, to-do with natural-language times, shelf (drag in/out, share),
  clipboard history with password-manager exclusions, prayer times (12 methods), Smart OCR,
  maths/units/currency, notification mirror + history, mascot, sounds.
- Ask (Ollama auto-detect, Gemini/OpenAI/Anthropic keys in DPAPI, streaming, Markdown, context chips),
  translate + translate-selection, command palette, weather (MET Norway), calendar (ICS, Google,
  Microsoft), desktop widgets, floating + glass styles, themes + theme packs + adaptive colour.
- Live Activities API (pipe, CLI, Python/Node SDKs, examples), privacy dots + history, meeting mode,
  audio visualiser, download tracker, Phone Drop, screenshots with annotation, system monitor,
  Bikram Sambat + world clocks, quick notes, settings sync.

### Shell
- Settings window (Mica, live preview, search, hotkey recorder, import/export), onboarding,
  tray menu, global hotkeys with conflict reporting, keyboard navigation + focus ring,
  UI Automation provider + announcements, English/Hindi/Nepali.
- Packaging: manifest + resources, MSIX and sparse-package manifests, Inno Setup script, update
  checker with SHA-256 verification, CI workflow.
