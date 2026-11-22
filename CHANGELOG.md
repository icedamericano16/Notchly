# Changelog

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
