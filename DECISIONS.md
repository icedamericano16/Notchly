# Decisions and deviations from the spec

Each entry: what was decided, and why.

## Product

- **Everything is free.** The owner decided to ship Notchly without a Pro tier. All
  licensing code (Store add-on, Ed25519 keys, Pro gate, locked previews, "Pro" chips) was
  removed rather than hidden, which also drops `ed25519-dalek` from the binary.

## Dependencies (beyond the spec's allowed list)

- `windows-future`, `windows-collections`, `windows-numerics`: split out of the `windows`
  crate in 0.62; required for WinRT async (`.join()`), collections and D2D matrices.
- **No `rustfft`, `tracing`, `fuzzy-matcher`:** a 40-line radix-2 FFT, a 3-file rolling
  logger and a small subsequence scorer cover the needs with less binary size.
- `qrcode` (allowed) for Phone Drop.

## Rendering & motion

- Icons are original SVG path strings parsed **at runtime** into cached D2D geometries
  instead of a `build.rs` conversion step — same crispness, no build-time tooling.
- Continuous corners use the well-known three-cubic "iOS 7" coefficients (extent 1.528 r)
  rather than the spec's two-cubic 1.28× approximation; the radius is clamped so the extended
  corner always fits.
- The dual-island gooey bridge vanishes once the bubble reaches its 8 DIP resting gap
  (the spec's 14 DIP threshold would leave a permanent neck at rest).
- Staggered rows use the closed-form spring step response, so they are still springs
  while being easy to offset per row.
- The first frame after idle uses at most 1/60 s so animations never start with a jump.
- **No system backdrop.** Since 0.3.0 every material is drawn by Notchly itself over a
  transparent window; see "The grey rectangle" below for why Windows' blur was dropped.

## Liquid Glass (0.2.0)

- **M0 spike (2026-11-23), Windows 11 build 26300, RTX 4060 Laptop, 1920×1200 at 125 %:**
  a topmost test window with `WDA_EXCLUDEFROMCAPTURE` was absent from both the Desktop
  Duplication image (0 of 33,600 test pixels; 33,600 of 33,600 without the flag, as a control)
  and a GDI screen grab. Copying a 1080×660 region on the GPU took 0.015 ms and a σ = 45 blur of
  it 0.034 ms (timestamp queries). **Decision: build Tier A on Desktop Duplication.** Not tested:
  Windows 10 (no machine; the code path is the same API and `WDA_EXCLUDEFROMCAPTURE` needs build
  19041, which tier selection checks) and integrated GPUs.
- **Trade-off, kept as a setting:** excluding the notch from capture also hides it from
  screenshots, recordings and shares. *Show Notchly in screen recordings* switches to the system
  blur tier. While capture is running the window stays excluded even if Liquid has fallen back
  (so it can never capture itself when it recovers). `Windows.Graphics.Capture` was not used: it
  honours the same exclusion and adds a WinRT frame pool for no gain here.
- **Windows reports an excluded window's own presents as screen changes**, both as a dirty
  rect equal to the window and as a whole-desktop recomposition, which made the glass redraw
  itself forever. Dirty rects equal to the notch's recent window rects are ignored, and every
  candidate frame goes through a GPU difference → histogram (D2D Blend difference, ColorMatrix,
  Histogram; only 64 bins are read back, on the capture thread) so the renderer is only woken
  when the pixels behind the notch actually changed. Unchanged checks back off from 30 Hz to
  5 Hz. Measured afterwards: static backdrop 0 frames presented, 0.06–0.10 % process CPU;
  animated backdrop 29 backdrop frames/s, 0.6–0.8 % CPU, 0.36–0.49 ms GPU per frame (dGPU).
- **Hand-over:** the capture thread has its own D3D11 device on the display's adapter and
  shares a keyed-mutex texture with the render device; the renderer copies new frames into a
  private texture before `BeginDraw`. A display driven by another adapter (hybrid GPUs),
  rotated displays, secure desktops and `ACCESS_LOST` fall back to system blur with backoff
  (1, 2, 4 ... 30 s) and recover by themselves.
- **No custom shaders.** Blur, lens (DisplacementMap from a CPU-built signed-distance lens map
  at 2 DIP per texel, cached by shape and redrawn at the context DPI so every effect input
  shares one pixel grid), colour lift (ColorMatrix), depth (Shadow on stroke command lists) and
  change detection are all built-in Direct2D effects, so there is no build-time HLSL step. The
  optional chromatic split on the rim was left out: it would need a custom effect or three extra
  displacement passes, for a ≤ 0.6 px effect.
- **σ range is 4–42 DIP** (spec example 18–42) so *Crystal* can be genuinely clear; strength
  scales σ by 0.75–1.15.
- **Vibrancy** is a tint floor for the whole glass (not per text run): with almost every pixel
  of the island being text or icons, a per-run scrim would look patchy. The floor keeps primary
  text ≥ 4.5:1 and lets secondary text (theme `mist`) brighten up to halfway to primary to reach
  4.5:1; tertiary (`FAINT`, decorative) is not adapted. The brightest of an 8×8 grid of the
  processed backdrop is sampled every 250 ms (GPU downscale; read back a frame later, no
  stall). The floor moves in 0.06 steps with hysteresis so a moving backdrop doesn't keep the
  island animating. The system blur tier can't be measured and uses a fixed floor.
- **Battery saver → Solid** for both glass tiers (Windows itself drops acrylic on battery saver).
- **Settings window** keeps Mica; its live preview runs the real compositor over a procedural
  sample wallpaper (no image shipped). "Over the real desktop" in the preview was not done: the
  Settings window would have to be excluded from capture as well.
- **Icons:** the 97-icon audit (sheet at 100/125 %) found consistent weight and optical size;
  the fix was pixel-snapping icon boxes and weight-tracked strokes rather than redrawing paths.
- **Fonts:** no bundled font (the optional Inter would cost ~300 KB); Segoe UI Variable is used
  through its axes where `IDWriteFactory6` and the font exist, else the named families as before.
- **Debug-only QA aids** (compiled out of release builds): `NOTCHLY_QA_DUMP` saves a rendered
  frame (the Liquid tier hides the notch from screenshots), `NOTCHLY_QA_IGNORE_SAVER`,
  `NOTCHLY_QA_SECTION`, `NOTCHLY_QA_ICONS`; `examples/glass_backdrop.rs` draws a test pattern.

## Platform

- **Brightness slider** writes the active power scheme's brightness (AC + DC) instead of
  WMI `WmiSetBrightness` — same effect on laptops, no WMI COM plumbing.
- **Equaliser** uses `IAudioMeterInformation` peaks in the simple case and the WASAPI loopback
  FFT when the visualiser runs.
- **Drag pre-open** triggers when a drag enters the notch window (it swells), not 80 DIP below
  it: detecting drags outside our window would need a global hook during every drag.
- **Screen-sharing detection** treats an active meeting as possible sharing (conservative:
  notification text is hidden during calls when the setting is on).
- **Notifications** use `UserNotificationListener.NotificationChanged`; if the event is refused
  in a given configuration the service falls back to a 1.5 s poll of new toast ids.
- **Phone Drop is plain HTTP on the LAN** (random port, 6-digit PIN, subnet check, 5-minute
  idle stop) instead of HTTPS with a pinned self-signed certificate: a TLS server would need an
  SChannel implementation. Documented in the privacy policy; opt-in.
- **Windows on-device AI** was removed from the provider list: those APIs need the Windows App
  SDK. The settings value is kept (mapped to Auto) so older settings files still load.
  Ollama (local) is the default provider; Gemini/OpenAI/Anthropic use your own keys.
- **Calendar OAuth clients are baked in at build time** (`NOTCHLY_GOOGLE_CLIENT_ID/SECRET`,
  `NOTCHLY_MS_CLIENT_ID`). Microsoft uses a public client with PKCE (no secret). Google's desktop
  clients require a secret that Google itself documents as non-confidential, so it ships in the
  build; PKCE still protects each sign-in. Users can override both with their own IDs.
- **Publisher-specific values come from the build, not the source** (`src/build_info.rs`):
  update feed, website, support link, OAuth clients, signing subject. A build without them
  works and simply hides the matching links / checks.
- **Live API activities outlive the connection** (the CLI disconnects immediately); orphaned
  activities expire after 10 minutes unless updated or ended.
- **QR decoding** uses `rqrr` (pure Rust, `image` feature disabled).
- **Fonts:** SF Pro can't be redistributed on Windows; Segoe UI Variable (Display/Text) is the closest
  system equivalent and is used throughout with tabular figures for numbers.

- **Notification history persistence** is opt-in and DPAPI-encrypted per Windows user, because
  toasts often contain message text.
- **Crash reports stay local**: Notchly has no server. The next start offers to copy the report or
  open the support link, so users decide what to share.
- **Sparse identity**: the exe's embedded manifest carries the `msix` identity element only when
  `NOTCHLY_PUBLISHER` is set, so unsigned/dev builds are unaffected.
- **Licence**: freeware (free to use, no redistribution of modified copies); replace LICENSE if
  the project should become open source.

## Data

- Bikram Sambat table: npdatetime `calendar_bs.csv` (BS 2000–2100), with 2062/2083/2087 taken
  from medic/bikram-sambat where the sources disagree. Verified anchors: 2000-01-01 = 1943-04-14,
  2080-01-01 = 2023-04-14, 2081-01-01 = 2024-04-13. Years after 2090 are published projections.
- Holidays are user-editable (`%APPDATA%\Notchly\holidays.json`, `[{"bs":"2083-01-01","name":"…"}]`);
  no festival list is bundled because lunar dates need yearly official data.
- NPR currency is derived from INR at the official 1.6 peg (ECB rates don't include NPR).
- Localization ships English, Hindi and Nepali for page titles, settings, pop-ups and
  onboarding; other strings fall back to English.

## The grey rectangle (0.3.0, M1)

Symptom: with *Glass* (Windows' blur, `DWMSBT_TRANSIENTWINDOW` + `DwmExtendFrameIntoClientArea(-1)`
+ a `CreatePolygonRgn` window region), a flat grey box filled the whole notch window around the
island. Diagnosed on Windows 11 build 26300 with `NOTCHLY_TRACE=glass` (window rect, region box,
point count, `SetWindowRgn` result, `GetWindowRgnBox` afterwards, DPI scale) over a white and a
striped test backdrop:

- **H2 (region lost / never re-applied): false.** `SetWindowRgn` succeeded every frame;
  `GetWindowRgnBox` reported a complex region the size of the island, e.g. (8,-1)-(737,374)
  inside a 746×384 window.
- **H3 (coordinate mismatch): false.** The region box lined up with the drawn island.
- **H4 (we paint the margin): false.** A dump of the swap chain is fully transparent outside the
  island.
- **H1 (inactive fallback): consistent, not provable.** The backdrop was flat (no colour from the
  stripes behind came through), also after an attempt to activate the window; Windows'
  foreground lock refuses activation from a background process, so the active case couldn't be
  tested without stealing focus, which is not acceptable anyway.
- **Cause: the DWM system backdrop ignores the window region** for this window (no redirection
  bitmap, a composition swap chain) and fills the whole window rectangle. A region still clips
  our own composition content and hit-testing, but not the backdrop. No documented API shapes it.

Fix (target architecture from the spec):

- **The system backdrop is gone.** *Glass* is now Notchly's own frosted glass from the same
  backdrop capture as Liquid Glass (no lens, at least σ 24, backdrop at most 20 Hz while still).
  Where the screen can't or shouldn't be read (battery saver, *Show Notchly in screen
  recordings*, Windows older than 2004, capture failing) the notch shows **tinted glass**: the
  full glass stack (tint, rim, inner shadow, sheen, a soft stand-in for the lens) over a
  transparent window, with a tint floor that keeps text at 4.5:1 even over white. Undocumented
  accent policies (`SetWindowCompositionAttribute`) were not used.
- **Click-through margins.** The window is layered and toggles `WS_EX_TRANSPARENT` by pointer
  position: outside the notch outline (grown by 2 DIP, plus the minimal-mode hover band) the
  mouse goes to the app underneath. `WM_NCHITTEST`/`HTTRANSPARENT` was not used: it only passes
  the mouse to windows of the same thread. A window region was tested and rejected: it does make
  hit-testing follow the shape, but it also clips our shadows, glows and tooltips. While the
  pointer is in a margin a 30 ms timer polls the cursor until it re-enters the notch or leaves
  the window, so there are no wake-ups otherwise. Known limit: the first mouse move into a
  margin is still delivered to Notchly (that is how it notices), later moves and clicks pass.
  Nothing is toggled while a press or drag is in progress. A layered window ignores the mouse
  until it has layered attributes, so the window gets `SetLayeredWindowAttributes(255,
  LWA_ALPHA)` at creation (the first M1 build missed this and the notch stopped opening on
  hover or click).
- **The window covers the whole visual reach** (canvas 700×420 DIP, margins 60 DIP floating /
  32 DIP attached), so the floating shadow and glow pulses are no longer cut off at the window
  edge (visible before as faint hard-edged bands). Bounds grow at once and shrink only after the
  shape's spring settles, so expansion never clips and fewer window moves happen mid-animation.
- **Materials cross-fade** on `GENTLE`: tint alpha, blur, lens, the captured backdrop (it fades in
  when the first frame arrives, out for tinted glass) and Solid's black.
- Brightness sampling (legibility floor) now runs for both capture tiers; before, Glass over a
  white window had too little tint.

## 0.3.0 polish round

- **Glass vs Liquid Glass.** With default sliders both blurred at σ≈25 with the same tint; only
  a few-DIP lens differed, so they looked the same. Glass now uses its own σ range (30–48), a
  white haze drawn *under* the tint (so the legibility floor still governs text contrast; the
  floor and vibrancy include the haze), a tiled 96-texel acrylic grain (7 %), saturation 1.04,
  and an even rim (0.26 / 0.18). Liquid Glass uses σ 3–26, a lens of up to 13 DIP across a
  12–20 DIP band, a lower tint table, and a counter-highlight on the far edge (0.30). All
  parameters blend through `mix_lens`, so switching material glides.
- **Soft shelf landing** is a damped spring started with the landing speed (2.1 Hz, ζ 0.36),
  mapped mostly to squash (0.0135 per DIP, widening 60 % as much as it shortens) and a little
  sink, with the rebound lifting 1.5× — one visible hop, settled by ~0.85 s after touchdown.
  The fall is 30 DIP so the card isn't hidden behind the header for most of it.
- **Idle pill**: both sides reserve the wider side's width, so the clock is centred on the notch
  in every combination; privacy dots are part of the right side. The minute roll runs on the
  wall clock (frames only during the first 0.38 s of a minute), no extra state.
- **Mouse vs trackpad** is told apart by the wheel delta: anything that isn't a whole multiple
  of 120 (WHEEL_DELTA) is a precision device (touchpads, free-spinning wheels). A gesture
  continues from its own target for 450 ms so fast notches accumulate even though Windows
  reports the new volume later. Brightness (a power-scheme write) is written at most every
  90 ms while scrolling, always ending on the last value.
- **Airplane mode** uses Windows.Devices.Radios: every radio off, remembering which were on and
  restoring them. The tile also reads Windows' own airplane flag (HKLM RadioManagement
  `SystemRadioState`, read-only). Setting that flag itself needs the undocumented
  `IRadioManager` COM API, which isn't used.
- **Camera** writes the same per-user switches as Windows Settings ("Let apps access your
  camera" and "Let desktop apps access your camera": HKCU ConsentStore `webcam` /
  `webcam\NonPackaged` `Value` = Allow / Deny). It stops new uses; an app already streaming may
  keep the camera until it reopens it, which is why blocking opens the in-use sheet. The
  device-wide switch (HKLM) would need admin rights.
- **Closing an app** posts WM_CLOSE to its visible, unowned top-level windows (it can save or
  ask); Force quit (TerminateProcess) appears after 3 s, or at once if it has no window.
  Processes are matched to the consent-store key by image path (desktop apps) or by the
  WindowsApps install folder's name and publisher id (packaged apps). Anything under %WINDIR%
  and `Microsoft.Windows.*` packages is never offered; Notchly never closes itself.
- Keep awake moved from the switches to the quick actions rather than being dropped.
- Debug-only QA hooks: `--qa-drop`, `NOTCHLY_QA_GLYPHS`, `NOTCHLY_QA_SHEET`,
  `NOTCHLY_QA_DUMP_EVERY` / `_COUNT` (filmstrips).

## Camera / microphone detection (0.3.0)

- On Windows 11 26H2 (build 26300) the CapabilityAccessManager no longer writes
  `LastUsedTimeStart/Stop` to HKCU: WhatsApp's keys had no usage values during a video call
  and the newest value of any app was a month old. Usage moved to
  `%ProgramData%\Microsoft\Windows\CapabilityAccessManager\CapabilityAccessManager.db`, which
  needs admin rights to read.
- **Microphone**: every active endpoint's `IAudioSessionManager2` sessions in the
  `AudioSessionStateActive` state (system sounds and PID 0 skipped), polled every 1.5 s.
  Verified with a helper that holds a capture stream open (detected within one poll, gone
  after it stopped).
- **Camera**: `MFCreateSensorActivityMonitor` (Windows 10 1809+) reports the processes
  streaming each sensor; its callback wakes the privacy thread immediately. The monitor starts
  on this PC; detection during a real camera stream was not exercised (it would have turned
  the user's camera on).
- **Location** has no public live API; it stays on the consent store (works on older builds,
  not on 26H2). The consent store is also the fallback for camera / mic if the live sources
  can't start.
- Uses found in `msedgewebview2.exe` (and `webviewhost.exe`) are credited to the nearest
  non-helper ancestor, so WebView2 apps show and close as themselves. Names come from the
  exe's version info (`FileDescription`, then `ProductName`). Live uses are keyed by image
  path, so Close / Force quit match them exactly.
- Control Center: one 4-row grid (switches 128 DIP wide, an 8 DIP gap everywhere); the sliders
  span two rows instead of four; battery and quick actions became cards sharing the rows.

## 0.3.0 Phase 0 — stabilisation

- **Audit.** Idle (Solid, collapsed): 0.02 % CPU, no frames presented, handle count flat over
  ~16 s. Non-test `unwrap`/`expect` calls: only infallible ones remain (brush interface casts,
  a value set on the line above). Fixed: the diagnostic privacy scan leaked two event handles.
- **Long press** is a small state machine (`app/press.rs`) used for everything with a
  long-press action (today: the water bell). One outcome per press; right-click cancels a
  press. Threshold is `general.long_press_ms` (default 500, clamped 300–1500).
- **Camera / microphone detection.** The spec asked to verify the consent-store logic; it is
  kept and now tested with recorded data, but on Windows 26H2 the store no longer receives
  usage times (see "Camera / microphone detection (0.3.0)"), so live sources stay primary and
  the store is the fallback (and the only source for location). Known apps get proper names
  (Teams, Zoom, Chrome, Edge, Camera, WhatsApp …). A browser call shows as the browser: the
  consent store and the audio engine only know the process, not the site (Meet).
- **Debounce**: appear at once, disappear after 2.5 s missing.
- **Logs** moved from %APPDATA% to %LOCALAPPDATA%\Notchly\logs (machine-local; settings and
  data stay in %APPDATA%). Debug logging is opt-in (Settings › General).
- **Diagnostics** is a hidden page (not in the carousel): Ask › Diagnostics, Settings ›
  General › Diagnostics, or `--page diagnostics`.

## 0.3.0 Phase 1 — Settings 2.0

- **Settings v5.** The water settings moved out of `[productivity]` into their own `[water]`
  section (`reminders, goal_ml, glass_ml, start_min, end_min`, plus the new `style`,
  `snooze_min`, `stop_at_goal`). Migration is automatic and tested, as is a v1 file that
  carries something from every migration.
- **Validation.** Loading has three layers: a file that isn't TOML is renamed `.bad` and the
  defaults load (as before); a value of the wrong type or an unknown option falls back to its
  default *field by field* (its neighbours are kept, nested tables too); then
  `Settings::sanitize` brings numbers into the same ranges the controls offer and repairs empty
  names, shortcuts and reversed water hours. Import goes through the same path.
- **Reset.** Each section's header has *Reset section* (inline "Reset · Cancel", 4 s); Privacy ›
  Settings file has *Reset all settings*. Neither touches non-preferences: onboarding done,
  where the floating notch was dragged, the chosen music session, and (Calendar & weather) the
  OAuth client IDs, which are connection setup.
- **Undo** covers every change made in the window (50 steps, Ctrl+Z); a slider drag is one
  step. Text fields keep their own Ctrl+Z while focused.
- **Water page always in the carousel.** It used to vanish when reminders were off, which
  made the page's own bell a trap (turn it off and the page disappears).
- **Silent glow** (a reminder style): no pop-up and no chime; the idle notch's lower edge
  breathes in the water colour for 6 s (steady fade with reduced motion).
- **Sounds** (timer chime, reminder chime, interface sounds) existed in the model but had no
  rows; they are in Pop-ups › Sounds now.
- **Scope choices.** The preview's "Your desktop" uses the wallpaper file (decoded once, never
  saved) rather than a live capture, which would hide Settings from screen capture like the
  island. The corner-roundness slider and the Motion feel editors are theme / spring
  properties: they come with Theme Studio (Phase 2) and Fluid (Phase 3). The idle clock moved
  to Appearance › Details, where the spec lists it. Notch size stays a dropdown (five fixed
  sizes read better than a stepper).
- **Window**: 1040 × 720 DIP by default, at least 860 × 600, remembered per monitor in its own
  small file (`settings_window.toml`) so the place never lands in exported or synced settings.

## 0.3.0 Phase 2 — Theme Studio

- **ThemePack v2** adds `version`, `author`, `surface_raised`, `ink`, `mist` and glass defaults
  (`glass.strength`, `glass.blur`, 0–1). Every field is optional, so v1 files load unchanged;
  unknown fields are ignored; the 64 KB cap stays; ids must be file-safe (`a-z0-9-_`).
- **Your themes are files** in `themes/` (one per theme, named by id), so the folder, export
  and settings sync keep working. Built-ins (the presets and the five shipped packs) are
  read-only: the first edit makes "<Name> copy" and says so.
- **Editing** changes the active theme live; files are written 400 ms after the last change
  (and on close). Theme edits have their own undo / redo (50 steps, Ctrl+Z / Ctrl+Y) because
  they live in theme files, not in settings; a drag is one step.
- **Glass defaults.** A theme's opacity and blur are applied when the theme is chosen; moving
  the Appearance sliders afterwards still overrides them.
- **Contrast check** (inline, never blocking): body text must reach 4.5:1 on the surface, the
  raised surface and the fully tinted glass over white and dark; secondary text 3:1. *Fix*
  moves text lightness away from the surface, and deepens the glass tint and surfaces when
  even white or black text can't pass. Every built-in passes (tested).
- **Light / dark / follow Windows**: each mode has its own theme (Appearance › Mode, Theme,
  Light-mode theme); Follow Windows reads `SystemUsesLightTheme` and switches on the setting
  change. The notch itself stays dark in light mode — it imitates a camera cut-out — so
  light-mode themes are tuned for light desktops (brighter rims, glass that keeps white text
  legible on a white wallpaper). Two new built-ins for that: Daylight (the light-mode default)
  and Porcelain.
- **Theme switching** cross-fades accent, glass tint and rim in OKLab (the colour springs run
  in OKLab), so saturated colours never pass through grey.
- **Share codes** are `NTH2:` + base64url(JSON), validated exactly like a file.
- **Not built (out of scope for this pass)**: the colour-blind preview, the eyedropper,
  harmony suggestions, the gradient-position editor, a font list picker (the font is a text
  field), per-feature tint editing and spring editors (springs come with Phase 3).

## 0.3.0 Phase 3 — Fluid Styles

- **Engine** (`render/glass/fluid.rs`): a style is data — a list of layers (blobs on
  Lissajous paths, folding ribbons, sine bands, rising glints, a pointer-led highlight).
  `marks()` turns a style and a moment into plain shapes (pure and tested); `draw()` paints
  them inside the glass, under the content, clipped to the notch. Eight styles ship: Still,
  Ripple, Flow, Aurora, Lava, Ocean, Mercury, Bubbles; each suggests a built-in theme.
- **Drawn at full resolution with plain Direct2D brushes**, not an offscreen ½-resolution
  bitmap with blur/threshold effects: at most 32 shapes per frame (enforced), radial and
  linear gradient fills, which measured cheaper than an extra effect graph. The metaball
  merge and the CPU caustic texture are not built; Ocean uses glints for its light.
- **Pacing.** A style animates only with Liquid Glass, motion allowed (Reduce motion and
  Windows "Show animations" freeze it on its still frame), off battery saver, while the window
  is visible and — by default — not while the notch is idle (*Pause fluid when idle*). When
  only the fluid moves it asks for ≤ 30 frames a second (a deadline, not a busy loop); when the
  island animates it rides along at full rate. Glass (frosted) shows the still frame.
  Measured (debug build, Liquid Glass, 10 s idle): Still 0 fps / 0.3 %; Flow paused 1 fps /
  0.1 %; Aurora animating while idle ~22 fps / 0.4 % of all cores.
- **Reactions**: hover ×1.25 speed, music ×1.6 (when *React to music*), and a Peek arrival
  brightens the fluid 10 % for 350 ms and swells Ocean's waves (when *React to
  notifications*); presses and arrivals keep the glass's own ripple. Not built: beat-driven
  energy, the timer-progress rise for Lava, the night boost for Aurora, the custom style
  editor (§5.6) and the motion "feel" curve editors.
- **Global animation speed** is now a percentage (50–200 %, settings v6; Calm / Default /
  Fast migrate to 70 / 100 / 180). Springs' stiffness scales with its square so time scales
  linearly; the fluid's speed multiplies with it.

## 0.3.0 Phase 4 — release plumbing

- **Signing providers** (`packaging/build.ps1`), first match wins: self-signed test cert,
  Azure Trusted Signing, SignPath, a .pfx (from `-PfxPath` or the `NOTCHLY_PFX_BASE64` secret,
  decoded only into the runner's temp folder; the publisher is read from the certificate).
  Every signature uses SHA-256 with an RFC 3161 timestamp and is checked with
  `signtool verify /pa`. No certificate → a clear warning and an unsigned build, never a
  failure, so contributors can build releases locally.
- **Signed update manifest.** ECDSA P-256 over SHA-256, signature in IEEE P1363 (r‖s) form,
  stored next to the manifest as `update.json.sig` (base64). Keys are raw base64 (private
  D‖X‖Y, public X‖Y) rather than PEM so the scripts run on Windows PowerShell 5.1 as well as
  PowerShell 7. The app verifies with Windows CNG against the public key baked in at build
  time (`NOTCHLY_UPDATE_PUBKEY`) over the exact downloaded bytes, then checks the installer's
  SHA-256 from the signed manifest. A build without the key never auto-installs (it says so).
  A unit test verifies a signature produced by `sign-update.ps1`, so the two sides can't
  drift apart.
- **Feed location**: the latest GitHub Release's `update.json`
  (`…/releases/latest/download/update.json`), set by the workflow unless the
  `NOTCHLY_UPDATE_FEED` variable overrides it. No server to run.
- **Workflows**: `ci.yml` checks every push and PR; `release.yml` runs on `v*` tags (tag must
  equal the Cargo version), builds, signs when secrets exist, signs the manifest and
  publishes the release with the changelog section as notes.
- **OAuth** needed no code changes: Google uses the installed-app flow (PKCE S256, loopback on
  a random port, `state` check, `calendar.readonly`), Microsoft the identity platform v2
  `common` endpoint (PKCE, loopback, `Calendars.Read offline_access`, public client, no
  secret); refresh tokens live in Credential Manager; client IDs come from build config or
  Settings, never from source.
- **Config files**: `config.toml` (build values, read by build.rs; the old name
  `notchly.local.toml` still works) and `.env` (signing) are git-ignored; `config.example.toml`
  and `.env.example` hold placeholders only.
- **Privacy policy** gains the Google API Services User Data Policy / Limited Use
  statement Google's verification asks for; `docs/privacy.html` is generated from
  `docs/PRIVACY.md` (`packaging/privacy_html.py`) for GitHub Pages.

## 0.3.0 Phase 5 — manual testing

- `TESTING.md` is the pre-release checklist (expected result + pass/fail per row). Rows that
  need hardware or accounts — real calls, multi-monitor, DPI changes, sleep, fullscreen
  games, the update from an older signed build, calendar sign-in — can only be checked by a
  person; everything else is also covered by unit tests or the debug QA hooks
  (`NOTCHLY_QA_SECTION`, `_SEARCH`, `_SCROLL`, `_PICK`, `NOTCHLY_QA_DUMP_TARGET=settings`).
- **Size budget.** With Settings 2.0, Theme Studio and Fluid Styles the release exe reached
  4.19 MB under `opt-level = "s"`, over the 4 MB CI budget. `opt-level = "z"` brings it to
  3.77 MB. The budget stays; the frame work is Direct2D on the GPU, so the smaller code's
  slightly slower CPU paths (layout, springs) don't show.

## Production hardening (zero-cost release), Phase 1: keys and secrets

- Every key and outside service is classified in `docs/KEYS_AND_SECRETS.md`: Tier 1
  (public by design, injected at build time), Tier 2 (CI-only secrets) and Tier 3 (local,
  free no-key, or bring-your-own-key). Most of the policy was already true: OAuth is the
  installed-app flow (system browser, loopback, PKCE, `state`), refresh tokens live in
  Credential Manager, AI keys are DPAPI-encrypted, sun and prayer times are computed locally,
  and "use my own client ID" exists for Google and Microsoft.
- `config.example.toml` became `notchly.local.example.toml` (all values commented out,
  so copying it never bakes placeholders in). build.rs reads `notchly.local.toml` first,
  `config.toml` second.
- The User-Agent named `notchly.app`, a domain the project doesn't own; MET Norway's terms
  need a real contact, so it now points at the GitHub repository.
- gitleaks 8.30.1 over both repositories' full history: no leaks. Over the working trees
  (including ignored files): findings only in `target\` build output (the Tier 1 Google
  client values baked into binaries, and random-byte false positives) and the git-ignored
  `notchly.local.toml`. Nothing to rotate.

## Production hardening, Phase 2: release pipeline

- The Microsoft Store is the primary channel: it signs the MSIX at no cost, and package
  identity is what lets notification mirroring use `UserNotificationListener`. The GitHub
  `.exe` stays as the advanced, unsigned download with `SHA256SUMS.txt`; Azure Trusted Signing
  was removed (it needs a paid subscription). SignPath (free for OSI projects) is an optional
  step that is off unless `NOTCHLY_SIGNPATH_ENABLED` is set.
- `packaging/release-check.ps1` stops a release whose tag, Cargo version, CHANGELOG section,
  `update.json` (version, URL, SHA-256) or update-signing key pair disagree. It also refuses a
  section still marked "Unreleased". Release notes carry only that version's section.
- The repository has two releases (`v0.3.0` and a hand-made `Notchly` duplicate) on the same
  commit. Deleting a release or tag is the user's call (`HUMAN_STEPS.md`); the next release
  is 0.3.1, so v0.3.0 can stay as it is.

## Production hardening, Phase 3: bugs

- **Meetings.** Browser calls were all called "Google Meet", and detection used the
  foreground window only. `features/meeting.rs` names the provider from the titles of all
  of the browser's windows. A `Tracker` keeps a browser call alive while that browser
  holds the microphone, with a 6 s grace period re-checked from `tick` (privacy events
  arrive only on change).
- **Downloads.** Reproduced with a separate writer process: while a file is held open,
  NTFS sends no size or last-write notifications (one `Created` at 0 bytes, then nothing
  until close, 37 ms before the rename), so `Progress` was never sent. Notifications now
  only choose which temp files to follow. A per-folder poller reads their sizes every
  700 ms and blocks on a channel when nothing is followed, so there are no idle wake-ups.
- **City time zone.** The forecast (MET Norway) has no zone, but Open-Meteo geocoding returns
  the IANA name. `Windows.Globalization.Calendar.ChangeTimeZone` converts it with DST at
  no cost and with no bundled tz data. A one-time lookup fills it in for cities saved
  earlier, matched by distance (names like "Springfield" span zones).
- **Docs.** Sizes and test counts are no longer hard-coded; CI rejects them in the README
  and Store listing.

## Production hardening, Phase 4: calendar

- ICS is the default, zero-setup path for Outlook and everything else, so the parser
  (`services/ics.rs`) covers real exports: all four frequencies, BYDAY with ordinals,
  BYMONTHDAY, BYMONTH, EXDATE, RECURRENCE-ID, CANCELLED, DURATION, and TZID. Recurrences are
  expanded in the event's wall clock and converted per occurrence.
- Outlook writes Windows zone names, which WinRT rejects. The ICU that ships with Windows
  (`icu.dll`, `ucal_getTimeZoneIDForWindowsID`) maps them. It is loaded at run time rather
  than imported, so the exe doesn't gain a load-time dependency.
- Settings hide Connect rows that could only fail and show the ICS route instead.
- Google scopes are `calendar.calendarlist.readonly` + `calendar.events.readonly`, the
  narrowest pair for the two calls made. Existing tokens keep working.

## Production hardening, Phase 5: robustness and structure

- **No unwrap/expect outside tests.** 36 of the 40 were `cast().expect("brush")`, a COM
  QueryInterface for an upcast. `canvas::as_brush` uses the infallible `From` that windows-rs
  generates for interface hierarchies instead. The other four were restructured so the
  failing case can't occur.
- **Unsafe code.** Writing SAFETY comments found real issues: unaligned struct reads from
  `Vec<u8>` (DIB header, TOKEN_USER, PDH items), an unbounded NUL scan over dropped text, a
  leaked token handle and a `static mut`. Each is fixed. Raw-pointer dereferences,
  `from_raw_parts` and `transmute` carry a SAFETY comment. Plain FFI calls with owned
  arguments don't; there are about 400 and the comments would add noise without information.
- **Crash reports.** One notice on the next start: a pre-filled GitHub issue (user names
  redacted from paths, backtrace shortened for the URL limit), opened in the browser so the
  user decides. No dialog inside the panic hook (`panic = "abort"`).
- **Phone Drop.** The QR holds a one-time 128-bit token (2 min) exchanged for an `HttpOnly`,
  `SameSite=Strict` cookie; the PIN is only a typed fallback. Still plain HTTP on the LAN (see
  the earlier entry), now labelled "trusted Wi-Fi only".
- **Refactor.** The three largest files are split by moving cohesive method groups into child
  modules that start with `use super::*`. Code moves unchanged, with only `pub(super)` added,
  and each step built, linted and passed the tests on its own. `events.rs` went from 3305 to
  2101 lines, `main.rs` from 2993 to 1960 and `settings/window.rs` from 5151 to 1713.
  `dispatch` (one 850-line match) stays whole: splitting a match is where behaviour changes
  slip in.

## Production hardening, Phase 6: website (Firebase Spark only)

- **Release data at build time.** `scripts/release-data.mjs` fetches the releases, the star
  count, `CHANGELOG.md` and `docs/PRIVACY.md` once per build. Browsers make no GitHub API
  calls (no rate limits, no third-party request), and deploys fail rather than publish
  without downloads. The data every page loads is 2.5 KB; changelog and policy text load
  only on their pages.
- **One source of truth.** `/changelog` is `CHANGELOG.md` by version: GitHub release bodies
  repeated the whole changelog, and the duplicate "Notchly" release doubled 0.3.0.
  `/privacy` renders `docs/PRIVACY.md` in full. The exe size comes from the release's own
  `notchly.exe`.
- **Store first when it exists.** `VITE_STORE_URL` turns on the Store button. Without it
  there's no dead button, just the GitHub installer, labelled unsigned with its SHA-256.
- **Measuring honestly.** `vite preview` falls back to the home page (hydration mismatch),
  and an uncompressed HTTP/1.1 localhost makes Lighthouse model slow, multi-connection
  loads. So the smoke tests use `scripts/serve-dist.mjs`, which routes and compresses like
  Firebase, and the performance target was checked on a Firebase preview channel and the
  live site. Mobile, live: home 93, download 98; accessibility, best practices and SEO 100.
  The real fixes were: the largest hero capture first (a bigger one later reset LCP to
  12 s+), static page titles (opacity-0 entrances don't count as LCP), the first feature
  group visible at first paint, and the app loader waiting one more frame.

## Production hardening, Phase 7: gates

- Both repositories run gitleaks over the full history in CI. The app adds a docs check for
  drifting numbers; the website adds unit tests and Playwright smoke tests. Neither
  workflow deploys.
- `TESTING.md` is the release checklist: section 0 lists what automation covers, and
  everything else is marked "needs manual verification".

## Production hardening, Phase 8: backlog

- **Narrator.** The UI Automation provider existed but kept its tree in a `thread_local`.
  UIA calls providers on its own threads, so screen readers saw an empty, unnamed pane.
  The tree is now process-wide behind a mutex. This was verified with Windows' UIA client:
  30 named buttons, and Invoke works.
- **Every monitor → "Follow the mouse".** A separate notch on every display at once would
  need several render windows; the app is built around one window (surface, glass
  capture, hit testing, UIA). Following the pointer covers the need (the notch is where
  you work) without that rewrite. A cheap handle check every 250 ms runs only when the
  option is chosen. It has not been tested on real multiple displays here (one display);
  see TESTING 4.F.
- **Theme gallery as NTH2 codes.** The gallery is a plain text file of share codes,
  decoded with the same validation as a pasted code. It is offline, needs no server, and
  adding a theme means pasting a line. Tests guard contrast and clashes with built-in
  names and ids.
- **Notification rules.** Four rules (show, quiet, muted, priority) on top of the existing
  muted list. Priority uses the existing "important" path (priority 90) rather than a new
  bypass.
- **Scoop.** It uses the portable exe, so the in-app updater stands down under
  `\scoop\apps\` (it would install a second copy).
- **Hindi and Nepali pages.** There is one landing page per language rather than a fully
  translated site. The install guide and changelog change with every release, and
  translating them would go stale. The app is translated already; the pages say what
  stays in English.
- **First-run tour.** The tour is added after the existing setup steps instead of
  replacing them, and the finish step is now explicit (99), not "3".
