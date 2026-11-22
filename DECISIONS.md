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
- **Glass** uses the DWM system backdrop (`DWMWA_SYSTEMBACKDROP_TYPE`, transient window)
  shaped with a rounded window region, instead of a Windows.UI.Composition host-backdrop brush
  (which would require moving the whole visual tree to WinRT composition). Falls back to solid
  on Windows 10, battery saver, transparency off and Remote Desktop.

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
- **Windows on-device AI** is listed but reports unavailable: those APIs need the Windows App
  SDK. Ollama (local) is the default provider; Gemini/OpenAI/Anthropic use your own keys.
- **Calendar OAuth** needs your own Google / Microsoft client IDs (Settings › Calendar & weather);
  shipping shared client secrets in a desktop app isn't possible safely.
- **Live API activities outlive the connection** (the CLI disconnects immediately); orphaned
  activities expire after 10 minutes unless updated or ended.
- **QR decoding** uses `rqrr` (pure Rust, `image` feature disabled).
- **Fonts:** SF Pro can't be redistributed on Windows; Segoe UI Variable (Display/Text) is the closest
  system equivalent and is used throughout with tabular figures for numbers.

## Data

- Bikram Sambat table: npdatetime `calendar_bs.csv` (BS 2000–2100), with 2062/2083/2087 taken
  from medic/bikram-sambat where the sources disagree. Verified anchors: 2000-01-01 = 1943-04-14,
  2080-01-01 = 2023-04-14, 2081-01-01 = 2024-04-13. Years after 2090 are published projections.
- Holidays are user-editable (`%APPDATA%\Notchly\holidays.json`, `[{"bs":"2083-01-01","name":"…"}]`);
  no festival list is bundled because lunar dates need yearly official data.
- NPR currency is derived from INR at the official 1.6 peg (ECB rates don't include NPR).
- Localization ships English, Hindi and Nepali for page titles, settings, pop-ups and
  onboarding; other strings fall back to English.
